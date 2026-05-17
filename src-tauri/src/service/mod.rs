mod idle;
pub mod linux_session;
pub mod monitor;
pub mod process;
mod ocr;
pub mod rule;
mod screenlock;
pub mod screenshot;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::database::{self, Activity, Database};
use crate::{schema::AppState, config::privacy};

fn get_poll_interval_ms() -> u64 {
    #[cfg(target_os = "macos")]
    {
        1500
    }
    #[cfg(not(target_os = "macos"))]
    {
        500
    }
}

fn screen_lock_check_interval_ms() -> u64 {
    #[cfg(target_os = "macos")]
    {
        5000
    }
    #[cfg(not(target_os = "macos"))]
    {
        1000
    }
}

fn previous_app_backfill_duration(
    app_changed: bool,
    duration_to_record: i64,
    was_input_idle: bool,
    is_confirmed_idle: bool,
) -> i64 {
    if !app_changed || duration_to_record <= 0 || was_input_idle || is_confirmed_idle {
        0
    } else {
        duration_to_record
    }
}

fn backfill_previous_activity_if_needed(
    state: &Arc<Mutex<AppState>>,
    previous_activity: Option<&database::Activity>,
    duration_delta: i64,
    current_timestamp: i64,
    current_app_name: &str,
) {
    if duration_delta <= 0 {
        return;
    }

    let Some(previous_activity) = previous_activity else {
        return;
    };
    let Some(previous_id) = previous_activity.id else {
        return;
    };

    let state_guard = state.lock().unwrap_or_else(|e| e.into_inner());
    let _ = state_guard.db.merge_activity(
        previous_id,
        duration_delta,
        None,
        previous_activity.screenshot_path.as_deref().unwrap_or(""),
        current_timestamp,
    );
    log::debug!(
        "⏱️ 时长回补: {} +{}s (切换到 {})",
        previous_activity.app_name,
        duration_delta,
        current_app_name
    );
}

fn should_persist_merge_update(effective_duration: i64, keep_record_active: bool) -> bool {
    effective_duration > 0 || keep_record_active
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RecordingLoopDecision {
    should_continue: bool,
    screenshot_interval: u64,
    reset_capture_clock: bool,
}

/// 浏览器 URL 采集偶发失败时，尝试从最近同窗口标题的活动里恢复 URL。
/// 这是近似统计兜底：优先减少同一页面被切碎成多段或掉成 0 站点 0 页面。
fn recover_recent_browser_url(
    database: &Database,
    app_name: &str,
    window_title: &str,
    now_ts: i64,
    max_age_secs: i64,
) -> Option<String> {
    if !rule::is_browser_app(app_name) || window_title.is_empty() {
        return None;
    }

    database
        .get_latest_activity_by_app_title(app_name, window_title)
        .ok()
        .flatten()
        .and_then(|activity| {
            let age = now_ts - activity.timestamp;
            if age <= max_age_secs {
                activity.browser_url.filter(|url| !url.is_empty())
            } else {
                None
            }
        })
}

fn recording_loop_decision(is_recording: bool, screenshot_interval: u64) -> RecordingLoopDecision {
    if !is_recording {
        RecordingLoopDecision {
            should_continue: false,
            screenshot_interval: 1,
            reset_capture_clock: true,
        }
    } else {
        RecordingLoopDecision {
            should_continue: true,
            screenshot_interval,
            reset_capture_clock: false,
        }
    }
}

pub fn register_monitor_sevice(state: Arc<Mutex<AppState>>) {
    let cleanup_state_clone = state.clone();

    let ocr_semaphore = Arc::new(tokio::sync::Semaphore::new(2));

    let merge_screenshot_hash = Arc::new(std::sync::atomic::AtomicU64::new(0));

    let screen_lock_monitor = screenlock::ScreenLockMonitor::new();
    let mut last_screen_lock_check = std::time::Instant::now()
        .checked_sub(Duration::from_millis(screen_lock_check_interval_ms()))
        .unwrap_or_else(std::time::Instant::now);
    let mut cached_screen_locked = false;

    let screenshot_interval = 10;
    const IDLE_TIMEOUT_MINUTES: u64 = 5;
    // 5 分钟无操作认为是屏幕锁,这个值后续可以根据实际情况调整
    let idle_detector = idle::IdleDetector::new(IDLE_TIMEOUT_MINUTES);
    // 定时清理
    tauri::async_runtime::spawn(async move {
        // 每 12 小时检查一次
        let mut interval = tokio::time::interval(Duration::from_secs(12 * 3600));

        loop {
            interval.tick().await;
            // 计算 7 天前的时间戳
            let seven_days_ago = chrono::Local::now().timestamp() - (7 * 24 * 3600);

            println!(
                "🧹 Running cleanup for records older than timestamp: {}",
                seven_days_ago
            );

            let cleanup_guard = cleanup_state_clone.lock().unwrap();
            match cleanup_guard.db.cleanup_old_records(seven_days_ago) {
                Ok(paths) => {
                    let mut deleted_files = 0;
                    // 物理删除硬盘上的截图文件
                    for path in paths {
                        if std::fs::remove_file(&path).is_ok() {
                            deleted_files += 1;
                        }
                    }
                    println!(
                        "✨ Cleanup finished! Deleted {} old screenshots from disk.",
                        deleted_files
                    );
                }
                Err(e) => {
                    println!("❌ Cleanup database error: {}", e);
                }
            }
        }
    });

    // 监听窗口变化
    tauri::async_runtime::spawn(async move {
        println!("🚀 监控循环已启动");
        log::info!("🚀 监控循环已启动");
        let mut last_app_name: Option<String> = None;
        let mut last_window_title: Option<String> = None;
        let mut last_browser_url: Option<String> = None;
        let mut last_capture_time = std::time::Instant::now();
        // let mut last_db_update_time = std::time::Instant::now();
        let mut last_idle_log_time = std::time::Instant::now();
        let mut is_currently_idle = false;

        let poll_interval_ms = get_poll_interval_ms();
        loop {
            // 是否正在记录
            let decision = {
                let state_guard = state.lock().unwrap_or_else(|e| e.into_inner());
                recording_loop_decision(state_guard.is_recording, 1)
            };
            if decision.reset_capture_clock {
                last_capture_time = std::time::Instant::now();
            }
            if !decision.should_continue {
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }

            // 检查屏幕是否锁定
            if last_screen_lock_check.elapsed()
                >= Duration::from_millis(screen_lock_check_interval_ms())
            {
                cached_screen_locked = screen_lock_monitor.is_locked();
                last_screen_lock_check = std::time::Instant::now();
            }
            if cached_screen_locked {
                log::info!("🔒 屏幕已锁定，暂停活动统计");
                last_app_name = None; // 重置应用状态，解锁后视为新开始
                last_window_title = None; // 重置应用状态，解锁后视为新开始
                last_capture_time = std::time::Instant::now(); // 重置截图计时，避免解锁后累加锁屏时长
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }

            // 正式开始
            tokio::time::sleep(Duration::from_millis(poll_interval_ms)).await;

            let active_window_now = std::time::Instant::now();
            let cached_active_window = {
                let state_guard = state.lock().unwrap_or_else(|e| e.into_inner());
                reusable_cached_active_window(
                    state_guard.cached_active_window.as_ref(),
                    active_window_now,
                )
            };
            let mut active_window = if let Some(w) = cached_active_window {
                if rule::is_browser_app(&w.app_name) {
                    match monitor::get_active_window() {
                        Ok(nw) => nw,
                        Err(_) => w,
                    }
                } else {
                    w
                }
            } else {
                match monitor::get_active_window() {
                    Ok(nw) => nw,
                    Err(_) => {
                        last_capture_time = std::time::Instant::now();
                        continue;
                    }
                }
            };

            // 再次检查状态
            let should_capture = {
                let state_guard = state.lock().unwrap_or_else(|e| e.into_inner());
                state_guard.is_recording
            };

            if !should_capture {
                continue;
            }
            {
                if rule::should_skip_transient_window(&active_window) {
                    log::debug!("跳过系统瞬态进程: {}", active_window.app_name);
                    last_app_name = None;
                    last_window_title = None;
                    last_browser_url = None;
                    last_capture_time = std::time::Instant::now();
                    continue;
                }
            }
            {
                if rule::should_skip_system_window(&active_window) {
                    log::debug!(
                        "跳过系统/桌面窗口: {} (title={}, minimized={})",
                        active_window.app_name,
                        active_window.window_title,
                        active_window.is_minimized
                    );
                    last_app_name = None;
                    last_window_title = None;
                    last_browser_url = None;
                    last_capture_time = std::time::Instant::now();
                    continue;
                }
            }

            // 是否应在变更检测前探测浏览器URL
            let should_probe_browser_url = rule::should_probe_browser_url_before_change_detection(
                &active_window.app_name,
                &active_window.window_title,
                last_app_name.as_deref(),
                last_window_title.as_deref(),
                active_window.browser_url.as_deref(),
            );
            if should_probe_browser_url {
                if let Some(resolved_url) = monitor::resolve_browser_url_for_window(
                    &active_window.app_name,
                    &active_window.window_title,
                ) {
                    if last_browser_url.as_deref() != Some(resolved_url.as_str()) {
                        log::debug!(
                            "浏览器 URL 预探测命中: {} | {} -> {}",
                            active_window.app_name,
                            active_window.window_title,
                            resolved_url
                        );
                    }
                    active_window.browser_url = Some(resolved_url);
                }
            }

            // 浏览器 URL 存在瞬时采集失败时，尽量复用同窗口最近一次成功值，减少统计断裂。
            const BROWSER_URL_STICKY_GAP_SECS: i64 = 120;
            if active_window.browser_url.is_none()
                && rule::is_browser_app(&active_window.app_name)
                && !active_window.window_title.is_empty()
            {
                let now_ts = chrono::Local::now().timestamp();

                let recovered_url = if last_app_name.as_deref()
                    == Some(active_window.app_name.as_str())
                    && last_window_title.as_deref() == Some(active_window.window_title.as_str())
                {
                    last_browser_url.clone()
                } else {
                    let state_guard = state.lock().unwrap_or_else(|e| e.into_inner());
                    recover_recent_browser_url(
                        &state_guard.db,
                        &active_window.app_name,
                        &active_window.window_title,
                        now_ts,
                        BROWSER_URL_STICKY_GAP_SECS,
                    )
                };

                if let Some(recovered_url) = recovered_url {
                    log::debug!(
                        "恢复浏览器 URL: {} | {} -> {}",
                        active_window.app_name,
                        active_window.window_title,
                        recovered_url
                    );
                    active_window.browser_url = Some(recovered_url);
                }
            }

            let previous_window_title = last_window_title.clone();
            let previous_browser_url = last_browser_url.clone();

            let mut url_changed = match (&last_browser_url, &active_window.browser_url) {
                (Some(l), Some(r)) => l != r,
                (None, None) => false,
                _ => true,
            };

            // 标题变更
            let title_changed = match (&last_window_title, &active_window.window_title) {
                (Some(last_title), active_title) => last_title != active_title,
                (None, _) => true,
            };
            // 进程名变更
            let mut app_changed = match &last_app_name {
                Some(last) => last != &active_window.app_name || url_changed || title_changed,
                None => true,
            };

            let capture_min_interval_ms = rule::browser_change_capture_min_interval_ms(
                &active_window.app_name,
                title_changed,
                url_changed,
            );

            // 计算距离上次截图的时间
            let elapsed_since_capture = last_capture_time.elapsed();
            let elapsed_secs = elapsed_since_capture.as_secs();

            // 确认应用切换时，记录切换信息
            if app_changed && last_app_name.is_some() {
                log::info!(
                    "📊 应用切换: {} [{}] → {} [{}]",
                    last_app_name.as_deref().unwrap_or("无"),
                    previous_window_title.as_deref().unwrap_or(""),
                    &active_window.app_name,
                    &active_window.window_title,
                );
            }

            // 空闲检测
            let input_idle_seconds = idle_detector.get_idle_seconds();
            let input_idle = input_idle_seconds >= IDLE_TIMEOUT_MINUTES * 60;

            let was_input_idle = is_currently_idle;
            if last_idle_log_time.elapsed() >= Duration::from_secs(30) {
                if input_idle != is_currently_idle {
                    if input_idle {
                        log::info!("⏸️  键鼠超时，等待截图确认空闲状态...");
                    } else {
                        log::info!("▶️  检测到用户活动，恢复正常记录");
                        idle_detector.reset();
                    }
                }
                last_idle_log_time = std::time::Instant::now();
            }
            is_currently_idle = input_idle;

            // 截图
            // 1. 定时触发：到达配置的间隔时间
            // 2. 应用切换触发：需满足最小间隔
            let should_take_screenshot = if elapsed_secs >= screenshot_interval {
                log::debug!("定时截图触发");
                true
            } else if app_changed && elapsed_since_capture.as_millis() >= capture_min_interval_ms {
                if capture_min_interval_ms < MIN_CAPTURE_INTERVAL_MS {
                    log::debug!("浏览器导航截图触发");
                } else {
                    log::debug!("应用切换截图触发");
                }
                true
            } else {
                false
            };

            // 保存 app_name 副本供浮动窗口检测使用（在 move 之前）
            let frontmost_app_name = active_window.app_name.clone();

            if !should_take_screenshot {
                // 如果是因为冷却时间未到而没有截图，但应用/标签页实际上已经变化了
                // 那么我们不要更新 last_* 变量，这样下一个轮询周期 app_changed 仍然为 true
                if !app_changed {
                    last_app_name = Some(active_window.app_name.clone());
                    last_window_title = Some(active_window.window_title.clone());
                    last_browser_url = active_window.browser_url.clone();
                }
                continue;
            }
            if rule::should_refresh_browser_url_before_record(
                &active_window.app_name,
                &active_window.window_title,
            ) {
                if let Some(resolved_url) = monitor::resolve_browser_url_for_window(
                    &active_window.app_name,
                    &active_window.window_title,
                ) {
                    if active_window.browser_url.as_deref() != Some(resolved_url.as_str()) {
                        log::debug!(
                            "浏览器 URL 落库前刷新: {} | {} -> {}",
                            active_window.app_name,
                            active_window.window_title,
                            resolved_url
                        );
                    }
                    active_window.browser_url = Some(resolved_url);
                }
                url_changed = match (&last_browser_url, &active_window.browser_url) {
                    (Some(l), Some(r)) => l != r,
                    (None, None) => false,
                    _ => true,
                };
                app_changed = match &last_app_name {
                    Some(last) => last != &active_window.app_name || url_changed || title_changed,
                    None => true,
                };
            }

            // 保存切换前的应用名，用于时长归属修正
            let previous_app_name = if app_changed {
                last_app_name.clone()
            } else {
                None
            };

            // 取决定截图后，才更新上一个应用的信息
            last_app_name = Some(active_window.app_name.clone());
            last_window_title = Some(active_window.window_title.clone());
            last_browser_url = active_window.browser_url.clone();

            // 更新截图时间
            last_capture_time = std::time::Instant::now();

            // 使用距离上次截图的实际经过时间作为本次记录的时长
            // 而非固定的轮询间隔，避免截图间隔大于轮询间隔时丢失时长
            let (privacy_action, duration_to_record) = {
                let state_guard = state.lock().unwrap_or_else(|e| e.into_inner());
                let action = state_guard.privacy_filter.check_privacy_full(
                    &active_window.app_name,
                    &active_window.window_title,
                    active_window.browser_url.as_deref(),
                );
                // elapsed_secs 是距离上次截图的真实秒数，确保时长不丢失
                let duration = elapsed_secs.max(1) as i64;
                (action, duration)
            };
            // 锁已释放

            let current_timestamp = chrono::Local::now().timestamp();
            let previous_activity_to_backfill = if app_changed {
                resolve_previous_activity_to_backfill(
                    &state,
                    previous_app_name.as_deref(),
                    previous_browser_url.as_deref(),
                    previous_window_title.as_deref(),
                )
            } else {
                None
            };

            let adjusted_duration = if app_changed {
                0i64
            } else {
                duration_to_record
            };
            use crate::config::privacy::PrivacyAction;
            let _: Option<database::Activity> = match privacy_action {
                PrivacyAction::Skip => {
                    log::debug!(
                        "完全跳过: {} - {}",
                        active_window.app_name,
                        active_window.window_title
                    );
                    None
                }
                PrivacyAction::Anonymize => {
                    log::debug!(
                        "内容脱敏: {} - {}",
                        active_window.app_name,
                        active_window.window_title
                    );
                    let classification = {
                        let state_guard = state.lock().unwrap_or_else(|e| e.into_inner());
                        rule::resolve_activity_classification(
                            &state_guard.config,
                            &active_window.app_name,
                            &active_window.window_title,
                            active_window.browser_url.as_deref(),
                        )
                    };
                    let anonymized_is_confirmed_idle =
                        should_confirm_idle(input_idle, input_idle_seconds, false, false);
                    let previous_effective_duration = previous_app_backfill_duration(
                        app_changed,
                        duration_to_record,
                        was_input_idle,
                        anonymized_is_confirmed_idle,
                    );
                    backfill_previous_activity_if_needed(
                        &state,
                        previous_activity_to_backfill.as_ref(),
                        previous_effective_duration,
                        current_timestamp,
                        &active_window.app_name,
                    );
                    let effective_duration = if anonymized_is_confirmed_idle {
                        log::debug!("空闲确认: 脱敏活动跳过时长记录");
                        0
                    } else {
                        adjusted_duration
                    };

                    if effective_duration <= 0 && !app_changed {
                        None
                    } else {
                        let activity = Activity {
                            id: None,
                            timestamp: current_timestamp,
                            app_name: active_window.app_name,
                            window_title: "[内容已脱敏]".to_string(),
                            screenshot_path: None,
                            ocr_text: None,
                            category: classification.base_category,
                            duration: effective_duration,
                            browser_url: None,
                            exe_path: active_window.exe_path,
                        };

                        // 短暂获取锁写入数据库
                        let state_guard = state.lock().unwrap_or_else(|e| e.into_inner());
                        match state_guard.db.insert_activity(&activity) {
                            Ok(_) => Some(activity),
                            Err(e) => {
                                log::error!("保存活动记录失败: {e}");
                                None
                            }
                        }
                    }
                }
                PrivacyAction::Record => {
                    let (classification, screenshots_enabled) = {
                        let state_guard = state.lock().unwrap_or_else(|e| e.into_inner());
                        (
                            rule::resolve_activity_classification(
                                &state_guard.config,
                                &active_window.app_name,
                                &active_window.window_title,
                                active_window.browser_url.as_deref(),
                            ),
                            // 假设所有应用都启用截图
                            true,
                        )
                    };
                    let category = classification.base_category.clone();

                    // 先检查是否有可合并的记录（在截屏之前判断，避免不必要的截图保存）
                    let latest_activity = {
                        let state_guard = state.lock().unwrap_or_else(|e| e.into_inner());
                        if let Some(url) = active_window
                            .browser_url
                            .as_deref()
                            .filter(|url| !url.is_empty())
                        {
                            state_guard
                                .db
                                .get_latest_activity_by_url(url)
                                .ok()
                                .flatten()
                        } else if rule::is_browser_app(&active_window.app_name)
                            && !active_window.window_title.is_empty()
                        {
                            state_guard
                                .db
                                .get_latest_activity_by_app_title(
                                    &active_window.app_name,
                                    &active_window.window_title,
                                )
                                .ok()
                                .flatten()
                        } else {
                            state_guard
                                .db
                                .get_latest_activity_by_app(&active_window.app_name)
                                .ok()
                                .flatten()
                        }
                    };

                    // "Unknown" 进程名不做合并：无法区分是哪个进程，强制新建
                    // 防止所有识别失败的进程时长累积到同一条记录导致统计失真
                    // 时间间隔超过 10 分钟也不合并：上午/下午用同一个 app 属于不同工作段
                    const MERGE_GAP_SECS: i64 = 600;
                    let is_merge = if let Some(ref latest) = latest_activity {
                        let mut merge = active_window.app_name != "Unknown"
                            && (current_timestamp - latest.timestamp) <= MERGE_GAP_SECS;

                        // 如果由于某种原因 browser_url 获取失败，但它确实是一个浏览器
                        // 我们必须强制让 window_title 完全相同才能合并，否则不同标签页的切换会被死死合并成一条记录。
                        if merge
                            && active_window.browser_url.is_none()
                            && rule::is_browser_app(&active_window.app_name)
                            && latest.window_title != active_window.window_title
                        {
                            merge = false;
                        }

                        merge
                    } else {
                        false
                    };

                    if is_merge {
                        // === 合并路径：不保存截图，只做 OCR ===
                        let latest = latest_activity.unwrap();
                        let latest_id = match latest.id {
                            Some(id) => id,
                            None => {
                                log::error!("合并活动记录缺少 id，跳过");
                                continue;
                            }
                        };
                        let previous_screenshot_path = latest.screenshot_path.clone();

                        // 截屏到内存，保存为临时文件供 OCR 使用
                        let screenshot_result = if screenshots_enabled {
                            let state_guard = state.lock().unwrap_or_else(|e| e.into_inner());
                            state_guard
                                .screenshot_service
                                .capture_for_window(Some(&active_window))
                                .ok()
                        } else {
                            None
                        };

                        // ===== 空闲检测第二阶段：截图哈希确认 =====
                        // 只有键鼠超时时才检查屏幕变化，避免正常使用时的额外计算
                        let screenshot_idle = if input_idle {
                            if let Some(ref screenshot) = screenshot_result {
                                let hash = screenshot::ScreenshotService::calculate_image_hash(
                                    &screenshot.path,
                                )
                                .unwrap_or(0);
                                idle_detector.confirm_idle_with_hash(hash)
                            } else {
                                false
                            }
                        } else {
                            // 有键鼠活动，重置空闲检测器
                            idle_detector.reset();
                            false
                        };
                        let is_confirmed_idle = should_confirm_idle(
                            input_idle,
                            input_idle_seconds,
                            screenshots_enabled,
                            screenshot_idle,
                        );
                        let previous_effective_duration = previous_app_backfill_duration(
                            app_changed,
                            duration_to_record,
                            was_input_idle,
                            is_confirmed_idle,
                        );
                        backfill_previous_activity_if_needed(
                            &state,
                            previous_activity_to_backfill.as_ref(),
                            previous_effective_duration,
                            current_timestamp,
                            &active_window.app_name,
                        );

                        // 如果确认空闲，跳过时长记录
                        let effective_duration = if is_confirmed_idle {
                            log::debug!("空闲确认: 跳过本次时长记录");
                            0
                        } else {
                            adjusted_duration
                        };

                        // 合并记录（不更新 screenshot_path，保留活动创建时的原始截图）
                        // 即使 effective_duration 为 0，也需要更新时间戳以保持记录活跃
                        let (latest_archive_path, ocr_input_path, temporary_ocr_source_path) =
                            if let Some(ref screenshot) = screenshot_result {
                                (
                                    Some(screenshot.path.clone()),
                                    screenshot
                                        .ocr_source_path
                                        .clone()
                                        .unwrap_or_else(|| screenshot.path.clone()),
                                    screenshot
                                        .ocr_source_path
                                        .clone()
                                        .filter(|path| path != &screenshot.path),
                                )
                            } else {
                                (None, PathBuf::new(), None)
                            };

                        let persisted_screenshot_path = previous_screenshot_path.clone();
                        let mut persisted_duration = latest.duration;

                        if should_persist_merge_update(effective_duration, true) {
                            let state_guard = state.lock().unwrap_or_else(|e| e.into_inner());
                            match state_guard.db.merge_activity(
                                latest_id,
                                effective_duration,
                                None,
                                previous_screenshot_path.as_deref().unwrap_or(""),
                                current_timestamp,
                            ) {
                                Ok(_) => {
                                    persisted_duration += effective_duration;
                                    log::info!(
                                        "✅ 合并成功: {} (id={}, 新时长={}s)",
                                        active_window.app_name,
                                        latest_id,
                                        latest.duration + effective_duration
                                    );
                                }
                                Err(e) => {
                                    log::error!("合并活动记录失败: {e}");
                                }
                            }
                        }

                        // 对截图执行 OCR；若已成功合并，则保留最新截图并清理旧截图
                        if let Some(screenshot) = screenshot_result {
                            let latest_capture_path =
                                latest_archive_path.unwrap_or_else(|| screenshot.path.clone());
                            let state_clone = state.clone();
                            let data_dir_clone = {
                                let state_guard = state.lock().unwrap_or_else(|e| e.into_inner());
                                state_guard.data_dir.clone()
                            };

                            let ocr_sem = ocr_semaphore.clone();
                            let merge_hash = merge_screenshot_hash.clone();

                            tokio::spawn(async move {
                                use std::sync::atomic::Ordering;

                                // 非阻塞获取 permit，满载时跳过 OCR 避免任务堆积
                                let _permit = match ocr_sem.try_acquire_owned() {
                                    Ok(p) => p,
                                    Err(_) => {
                                        log::debug!("OCR 并发已满，跳过合并路径 OCR");
                                        if let Some(temp_path) = temporary_ocr_source_path.clone() {
                                            let _ = std::fs::remove_file(&temp_path);
                                        }
                                        let _ = std::fs::remove_file(&latest_capture_path);
                                        return;
                                    }
                                };

                                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

                                // 计算哈希做去重判断
                                let current_hash =
                                    screenshot::ScreenshotService::calculate_image_hash(
                                        &latest_capture_path,
                                    )
                                    .unwrap_or(0);
                                let last_hash = merge_hash.swap(current_hash, Ordering::Relaxed);

                                let should_ocr = if last_hash != 0 {
                                    let similarity = screenshot::ScreenshotService::hash_similarity(
                                        last_hash,
                                        current_hash,
                                    );
                                    if similarity > 90 {
                                        log::debug!("合并截图相似度 {similarity}%，跳过 OCR");
                                        false
                                    } else {
                                        log::debug!("合并截图相似度 {similarity}%，执行 OCR");
                                        true
                                    }
                                } else {
                                    true
                                };

                                if should_ocr {
                                    let ocr_service = ocr::OcrService::new(&data_dir_clone);
                                    if let Ok(Some(ocr_result)) =
                                        ocr_service.extract_text(&ocr_input_path)
                                    {
                                        if !ocr_result.text.is_empty() {
                                            let filtered_text =
                                                ocr::filter_sensitive_text(&ocr_result.text);
                                            if let Ok(state_guard) = state_clone.lock() {
                                                let _ = state_guard.db.update_activity_ocr(
                                                    latest_id,
                                                    filtered_text.as_str(),
                                                );
                                                log::info!(
                                                    "OCR 完成(合并): 活动 {} 识别到 {} 个字符",
                                                    latest_id,
                                                    ocr_result.text.len()
                                                );
                                            }
                                        }
                                    }
                                }

                                if let Some(temp_path) = temporary_ocr_source_path {
                                    let _ = std::fs::remove_file(&temp_path);
                                }

                                let _ = std::fs::remove_file(&latest_capture_path);
                                log::debug!(
                                    "已删除仅用于合并 OCR 的临时截图: {latest_capture_path:?}"
                                );
                            });
                        }

                        Some(Activity {
                            id: Some(latest_id),
                            timestamp: current_timestamp,
                            app_name: active_window.app_name.clone(),
                            window_title: active_window.window_title,
                            screenshot_path: persisted_screenshot_path,
                            ocr_text: None,
                            category,
                            duration: persisted_duration,
                            browser_url: active_window.browser_url,
                            exe_path: active_window.exe_path,
                        })
                    } else {
                        // === 新建路径：正常截屏并保存 ===
                        if screenshots_enabled {
                            let screenshot_result = {
                                let state_guard = state.lock().unwrap_or_else(|e| e.into_inner());
                                state_guard
                                    .screenshot_service
                                    .capture_for_window(Some(&active_window))
                            };

                            match screenshot_result {
                                Ok(screenshot_result) => {
                                    // ===== 空闲检测第二阶段：截图哈希确认 =====
                                    let screenshot_idle = if input_idle {
                                        let hash =
                                            screenshot::ScreenshotService::calculate_image_hash(
                                                &screenshot_result.path,
                                            )
                                            .unwrap_or(0);
                                        idle_detector.confirm_idle_with_hash(hash)
                                    } else {
                                        idle_detector.reset();
                                        false
                                    };
                                    let is_confirmed_idle = should_confirm_idle(
                                        input_idle,
                                        input_idle_seconds,
                                        screenshots_enabled,
                                        screenshot_idle,
                                    );
                                    let previous_effective_duration =
                                        previous_app_backfill_duration(
                                            app_changed,
                                            duration_to_record,
                                            was_input_idle,
                                            is_confirmed_idle,
                                        );
                                    backfill_previous_activity_if_needed(
                                        &state,
                                        previous_activity_to_backfill.as_ref(),
                                        previous_effective_duration,
                                        current_timestamp,
                                        &active_window.app_name,
                                    );

                                    // 如果确认空闲，跳过时长记录（但仍创建活动记录以保持截图）
                                    let effective_duration = if is_confirmed_idle {
                                        log::debug!("空闲确认: 新活动时长设为 0");
                                        0
                                    } else {
                                        adjusted_duration
                                    };

                                    let (
                                        relative_path,
                                        archive_path,
                                        ocr_input_path,
                                        temporary_ocr_source_path,
                                        data_dir_clone,
                                    ) = {
                                        let state_guard =
                                            state.lock().unwrap_or_else(|e| e.into_inner());
                                        (
                                            state_guard
                                                .screenshot_service
                                                .get_relative_path(&screenshot_result.path),
                                            screenshot_result.path.clone(),
                                            screenshot_result
                                                .ocr_source_path
                                                .clone()
                                                .unwrap_or_else(|| screenshot_result.path.clone()),
                                            screenshot_result
                                                .ocr_source_path
                                                .clone()
                                                .filter(|path| path != &screenshot_result.path),
                                            state_guard.data_dir.clone(),
                                        )
                                    };

                                    let activity = Activity {
                                        id: None,
                                        timestamp: screenshot_result.timestamp,
                                        app_name: active_window.app_name.clone(),
                                        window_title: active_window.window_title,
                                        screenshot_path: Some(relative_path.clone()),
                                        ocr_text: None,
                                        category,
                                        duration: effective_duration,
                                        browser_url: active_window.browser_url,
                                        exe_path: active_window.exe_path,
                                    };

                                    let inserted = {
                                        let state_guard =
                                            state.lock().unwrap_or_else(|e| e.into_inner());
                                        state_guard.db.insert_activity(&activity)
                                    };

                                    match inserted {
                                        Ok(activity_id) => {
                                            log::info!(
                                                "📝 新建活动: {} (id={})",
                                                active_window.app_name,
                                                activity_id
                                            );

                                            // 异步 OCR（新建活动的截图已保存，不删除）
                                            let state_clone = state.clone();
                                            let ocr_sem = ocr_semaphore.clone();
                                            tokio::spawn(async move {
                                                // 非阻塞获取 permit，满载时跳过 OCR
                                                let _permit = match ocr_sem.try_acquire_owned() {
                                                    Ok(p) => p,
                                                    Err(_) => {
                                                        log::debug!(
                                                            "OCR 并发已满，跳过新建路径 OCR"
                                                        );
                                                        if let Some(temp_path) =
                                                            temporary_ocr_source_path.clone()
                                                        {
                                                            let _ =
                                                                std::fs::remove_file(&temp_path);
                                                        }
                                                        return;
                                                    }
                                                };

                                                tokio::time::sleep(
                                                    tokio::time::Duration::from_secs(1),
                                                )
                                                .await;

                                                let ocr_service =
                                                    ocr::OcrService::new(&data_dir_clone);

                                                if let Ok(Some(ocr_result)) =
                                                    ocr_service.extract_text(&ocr_input_path)
                                                {
                                                    if !ocr_result.text.is_empty() {
                                                        let filtered_text =
                                                            ocr::filter_sensitive_text(
                                                                &ocr_result.text,
                                                            );
                                                        if let Ok(state_guard) = state_clone.lock()
                                                        {
                                                            let _ =
                                                                state_guard.db.update_activity_ocr(
                                                                    activity_id,
                                                                    filtered_text.as_str(),
                                                                );
                                                            log::info!(
                                                        "OCR 完成(新建): 活动 {} 识别到 {} 个字符",
                                                        activity_id,
                                                        ocr_result.text.len()
                                                    );
                                                        }
                                                    }
                                                }

                                                if let Some(temp_path) = temporary_ocr_source_path {
                                                    let _ = std::fs::remove_file(&temp_path);
                                                }
                                            });

                                            Some(Activity {
                                                id: Some(activity_id),
                                                ..activity
                                            })
                                        }
                                        Err(e) => {
                                            log::error!("保存活动记录失败: {e}");
                                            let _ = std::fs::remove_file(&archive_path);
                                            if let Some(temp_path) = temporary_ocr_source_path {
                                                let _ = std::fs::remove_file(&temp_path);
                                            }
                                            None
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::error!("截屏失败: {e}");
                                    None
                                }
                            }
                        } else {
                            let is_confirmed_idle = should_confirm_idle(
                                input_idle,
                                input_idle_seconds,
                                screenshots_enabled,
                                false,
                            );
                            let previous_effective_duration = previous_app_backfill_duration(
                                app_changed,
                                duration_to_record,
                                was_input_idle,
                                is_confirmed_idle,
                            );
                            backfill_previous_activity_if_needed(
                                &state,
                                previous_activity_to_backfill.as_ref(),
                                previous_effective_duration,
                                current_timestamp,
                                &active_window.app_name,
                            );
                            let effective_duration = if is_confirmed_idle {
                                log::debug!("关闭截图后按输入空闲判定，新活动时长设为 0");
                                0
                            } else {
                                adjusted_duration
                            };

                            let activity = database::Activity {
                                id: None,
                                timestamp: current_timestamp,
                                app_name: active_window.app_name.clone(),
                                window_title: active_window.window_title,
                                screenshot_path: None,
                                ocr_text: None,
                                category,
                                duration: effective_duration,
                                browser_url: active_window.browser_url,
                                exe_path: active_window.exe_path,
                            };

                            let inserted = {
                                let state_guard = state.lock().unwrap_or_else(|e| e.into_inner());
                                state_guard.db.insert_activity(&activity)
                            };

                            match inserted {
                                Ok(activity_id) => {
                                    log::info!(
                                        "📝 新建无截图活动: {} (id={})",
                                        active_window.app_name,
                                        activity_id
                                    );
                                    Some(database::Activity {
                                        id: Some(activity_id),
                                        ..activity
                                    })
                                }
                                Err(e) => {
                                    log::error!("保存无截图活动记录失败: {e}");
                                    None
                                }
                            }
                        }
                    }
                }
            };

            // 发送事件到前端
            // if let Some(activity) = result {
            //     if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
            //         let _ = window.emit("screenshot-taken", &activity);
            //     }
            // }


        // ===== 浮动窗口（PiP 画中画）检测 =====
        // 检测 layer > 0 的浮动窗口（如视频小窗），为它们记录使用时长
        // 浮动窗口不截图（截图已由主活动管理），仅记录时长
        let overlay_windows = monitor::get_overlay_windows(&frontmost_app_name);
        for ow in &overlay_windows {
            // 隐私检查
            let ow_privacy = {
                let state_guard = state.lock().unwrap_or_else(|e| e.into_inner());
                state_guard
                    .privacy_filter
                    .check_privacy(&ow.app_name, &ow.window_title)
            };

            if ow_privacy == privacy::PrivacyAction::Skip {
                log::debug!("浮动窗口跳过(隐私): {}", ow.app_name);
                continue;
            }

            let overlay_is_confirmed_idle =
                should_confirm_idle(input_idle, input_idle_seconds, false, false);
            if overlay_is_confirmed_idle {
                log::debug!("浮动窗口空闲确认，跳过时长记录: {}", ow.app_name);
                continue;
            }

            let classification = {
                let state_guard = state.lock().unwrap_or_else(|e| e.into_inner());
                
                rule::resolve_activity_classification(
                    &state_guard.config,
                    &ow.app_name,
                    &ow.window_title,
                    ow.browser_url.as_deref(),
                )
            };
            let ow_category = classification.base_category.clone();
            let current_ts = chrono::Local::now().timestamp();
            let ow_duration = poll_interval_ms.div_ceil(1000) as i64;

            // 查找该应用的最近活动记录，尝试合并
            let latest = {
                let state_guard = state.lock().unwrap_or_else(|e| e.into_inner());
                state_guard
                    .db
                    .get_latest_activity_by_app(&ow.app_name)
                    .ok()
                    .flatten()
            };

            const OW_MERGE_GAP_SECS: i64 = 600;
            let can_merge = if let Some(ref act) = latest {
                ow.app_name != "Unknown" && (current_ts - act.timestamp) <= OW_MERGE_GAP_SECS
            } else {
                false
            };

            if can_merge {
                let act = latest.unwrap();
                if let Some(act_id) = act.id {
                    let state_guard = state.lock().unwrap_or_else(|e| e.into_inner());
                    match state_guard.db.merge_activity(
                        act_id,
                        ow_duration,
                        None,
                        act.screenshot_path.as_deref().unwrap_or(""),
                        current_ts,
                    ) {
                        Ok(_) => {
                            log::info!(
                                "🪟 浮动窗口合并: {} (id={}, +{}s, 总{}s)",
                                ow.app_name,
                                act_id,
                                ow_duration,
                                act.duration + ow_duration
                            );
                        }
                        Err(e) => log::error!("浮动窗口合并失败: {e}"),
                    }
                }
            } else {
                // 新建活动记录（无截图）
                let ow_title = if ow_privacy == privacy::PrivacyAction::Anonymize {
                    "[内容已脱敏]".to_string()
                } else {
                    ow.window_title.clone()
                };

                let activity = Activity {
                    id: None,
                    timestamp: current_ts,
                    app_name: ow.app_name.clone(),
                    window_title: ow_title,
                    screenshot_path: None,
                    ocr_text: None,
                    category: ow_category,
                    duration: ow_duration,
                    browser_url: None,
                    exe_path: ow.exe_path.clone(),
                };

                let state_guard = state.lock().unwrap_or_else(|e| e.into_inner());
                match state_guard.db.insert_activity(&activity) {
                    Ok(id) => {
                        log::info!(
                            "🪟 浮动窗口新建: {} (id={}, {}s)",
                            ow.app_name,
                            id,
                            ow_duration
                        );
                    }
                    Err(e) => log::error!("浮动窗口记录失败: {e}"),
                }
            }
        }
        }
    });
}

const ACTIVE_WINDOW_CACHE_MAX_AGE_MS: u64 = 1250;
const MIN_CAPTURE_INTERVAL_MS: u128 = 3000;
const ACTIVITY_INPUT_IDLE_HARD_STOP_MINUTES: u64 = 20;
const ACTIVITY_INPUT_IDLE_HARD_STOP_SECS: u64 = ACTIVITY_INPUT_IDLE_HARD_STOP_MINUTES * 60;
fn should_confirm_idle(
    input_idle: bool,
    input_idle_seconds: u64,
    screenshots_enabled: bool,
    screenshot_confirmed: bool,
) -> bool {
    if !input_idle {
        return false;
    }

    // 长时间无输入时直接切断时长，避免后台程序或动态页面无限续时。
    if input_idle_seconds >= ACTIVITY_INPUT_IDLE_HARD_STOP_SECS {
        return true;
    }

    if screenshots_enabled {
        screenshot_confirmed
    } else {
        true
    }
}

fn reusable_cached_active_window(
    cached: Option<&(std::time::Instant, monitor::ActiveWindow)>,
    now: std::time::Instant,
) -> Option<monitor::ActiveWindow> {
    let (sampled_at, active_window) = cached?;
    let age = now.checked_duration_since(*sampled_at)?;

    if age > Duration::from_millis(ACTIVE_WINDOW_CACHE_MAX_AGE_MS) {
        return None;
    }

    Some(active_window.clone())
}

fn resolve_previous_activity_to_backfill(
    state: &Arc<Mutex<AppState>>,
    previous_app_name: Option<&str>,
    previous_browser_url: Option<&str>,
    previous_window_title: Option<&str>,
) -> Option<database::Activity> {
    let Some(previous_app_name) = previous_app_name else {
        return None;
    };

    let state_guard = state.lock().unwrap_or_else(|e| e.into_inner());

    if let Some(previous_url) = previous_browser_url.filter(|url| !url.is_empty()) {
        state_guard
            .db
            .get_latest_activity_by_url(previous_url)
            .ok()
            .flatten()
    } else if rule::is_browser_app(previous_app_name) {
        previous_window_title
            .filter(|title| !title.is_empty())
            .and_then(|title| {
                state_guard
                    .db
                    .get_latest_activity_by_app_title(previous_app_name, title)
                    .ok()
                    .flatten()
            })
            .or_else(|| {
                state_guard
                    .db
                    .get_latest_activity_by_app(previous_app_name)
                    .ok()
                    .flatten()
            })
    } else {
        state_guard
            .db
            .get_latest_activity_by_app(previous_app_name)
            .ok()
            .flatten()
    }
}


