// 空闲检测模块
// 检测用户是否处于空闲状态（无键鼠操作且屏幕内容无变化）
// 用于解决"应用挂着不用但时长继续累加"的问题

#[cfg(target_os = "linux")]
use crate::linux_session::{current_linux_desktop_session, LinuxDesktopSession};
use std::sync::atomic::{AtomicU64, Ordering};

/// 上一次截图的哈希值
static LAST_SCREENSHOT_HASH: AtomicU64 = AtomicU64::new(0);
/// 连续屏幕无变化的次数
static NO_CHANGE_COUNT: AtomicU64 = AtomicU64::new(0);

/// 空闲检测器
pub struct IdleDetector {
    /// 空闲超时时间（秒）
    idle_timeout_secs: u64,
}

impl IdleDetector {
    /// 创建空闲检测器
    pub fn new(idle_timeout_minutes: u64) -> Self {
        Self {
            idle_timeout_secs: idle_timeout_minutes * 60,
        }
    }

    /// 第一阶段：检查键鼠是否超时
    /// 返回 true 表示键鼠超时（可能空闲），需要进一步用截图哈希确认
    pub fn is_input_idle(&self) -> bool {
        let idle_secs = get_idle_seconds();
        idle_secs >= self.idle_timeout_secs
    }

    /// 第二阶段：用截图哈希确认是否真的空闲
    /// 在截图后调用，传入当前截图的哈希
    /// 返回 true 表示真的空闲（屏幕也没变化），应该跳过时长记录
    pub fn confirm_idle_with_hash(&self, current_hash: u64) -> bool {
        let last_hash = LAST_SCREENSHOT_HASH.swap(current_hash, Ordering::Relaxed);

        // 首次无历史哈希，不判定为空闲
        if last_hash == 0 {
            NO_CHANGE_COUNT.store(0, Ordering::Relaxed);
            return false;
        }

        // 计算相似度
        let similarity = hash_similarity(last_hash, current_hash);

        if similarity < 95 {
            // 屏幕有变化，不是空闲（终端输出、视频播放等）
            NO_CHANGE_COUNT.store(0, Ordering::Relaxed);
            log::trace!("屏幕有变化 (相似度 {}%)，视为活跃", similarity);
            return false;
        }

        // 屏幕无变化，累计计数
        let count = NO_CHANGE_COUNT.fetch_add(1, Ordering::Relaxed) + 1;

        // 需要连续多次无变化才判定为空闲
        // 按 30 秒截图间隔计算，3 次约 1.5 分钟
        if count >= 3 {
            log::debug!("确认空闲: 屏幕连续 {} 次无变化", count);
            return true;
        }

        log::trace!("屏幕无变化 ({}/3)，继续观察", count);
        false
    }

    /// 重置空闲状态（用户有活动时调用）
    pub fn reset(&self) {
        NO_CHANGE_COUNT.store(0, Ordering::Relaxed);
    }

    /// 获取当前空闲秒数（供外部使用）
    #[allow(dead_code)]
    pub fn get_idle_seconds(&self) -> u64 {
        get_idle_seconds()
    }
}

impl Default for IdleDetector {
    fn default() -> Self {
        Self::new(3)
    }
}

/// 计算两个哈希的相似度（0-100）
fn hash_similarity(hash1: u64, hash2: u64) -> u32 {
    let xor = hash1 ^ hash2;
    let diff_bits = xor.count_ones();
    100 - (diff_bits * 100 / 64)
}

// ============== Windows 实现 ==============

#[cfg(target_os = "windows")]
fn get_idle_seconds() -> u64 {
    use std::mem::size_of;
    use winapi::um::sysinfoapi::GetTickCount;
    use winapi::um::winuser::{GetLastInputInfo, LASTINPUTINFO};

    unsafe {
        let mut lii = LASTINPUTINFO {
            cbSize: size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };

        if GetLastInputInfo(&mut lii) != 0 {
            let current_tick = GetTickCount();
            let idle_ms = if current_tick >= lii.dwTime {
                current_tick - lii.dwTime
            } else {
                (u32::MAX - lii.dwTime) + current_tick + 1
            };
            (idle_ms / 1000) as u64
        } else {
            0
        }
    }
}

// ============== macOS 实现 ==============

#[cfg(target_os = "macos")]
fn get_idle_seconds() -> u64 {
    // 使用 FFI 直接调用 CGEventSourceSecondsSinceLastEventType
    // 因为 core-graphics crate 不直接暴露这个函数
    use core_graphics::event_source::CGEventSourceStateID;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventSourceSecondsSinceLastEventType(
            state_id: CGEventSourceStateID,
            event_type: u32,
        ) -> f64;
    }

    // kCGAnyInputEventType = ~0 (所有输入事件类型)
    const K_CG_ANY_INPUT_EVENT_TYPE: u32 = !0u32;

    let idle_time = unsafe {
        CGEventSourceSecondsSinceLastEventType(
            CGEventSourceStateID::HIDSystemState,
            K_CG_ANY_INPUT_EVENT_TYPE,
        )
    };

    if idle_time >= 0.0 {
        idle_time as u64
    } else {
        0
    }
}

// ============== Linux 实现 ==============

#[cfg(target_os = "linux")]
fn get_idle_seconds() -> u64 {
    use std::process::Command;

    if matches!(
        current_linux_desktop_session(),
        LinuxDesktopSession::Wayland
    ) {
        let output = Command::new("dbus-send")
            .args([
                "--session",
                "--print-reply",
                "--dest=org.freedesktop.ScreenSaver",
                "/org/freedesktop/ScreenSaver",
                "org.freedesktop.ScreenSaver.GetSessionIdleTime",
            ])
            .output();

        if let Ok(result) = output {
            if result.status.success() {
                let stdout = String::from_utf8_lossy(&result.stdout);
                if let Some(idle_ms) = stdout
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .windows(2)
                    .find_map(|parts| match parts {
                        ["uint32", value] => value.parse::<u64>().ok(),
                        _ => None,
                    })
                {
                    return idle_ms / 1000;
                }
            }
        }
    }

    // X11 以及 Wayland 回退到 xprintidle
    let output = Command::new("xprintidle").output();
    match output {
        Ok(result) if result.status.success() => {
            let stdout = String::from_utf8_lossy(&result.stdout);
            let idle_ms: u64 = stdout.trim().parse().unwrap_or(0);
            idle_ms / 1000
        }
        _ => 0,
    }
}

// ============== 其他平台 ==============

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn get_idle_seconds() -> u64 {
    0
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_similarity() {
        assert_eq!(hash_similarity(0x1234567890ABCDEF, 0x1234567890ABCDEF), 100);
        assert_eq!(hash_similarity(0x0000000000000000, 0xFFFFFFFFFFFFFFFF), 0);
    }
}