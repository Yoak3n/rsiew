use crate::config::AppConfig;
use crate::utils::activity_classifier;
use super::normalize::normalized_app_rule_key;



/// 根据应用名自动分类
pub fn categorize_app(app_name: &str, window_title: &str) -> String {
    let app_lower = app_name.to_lowercase();

    // 开发工具（IDE、编辑器、终端、数据库工具、API 工具、容器、版本控制）
    if app_lower.contains("code")
        || app_lower.contains("visual studio")
        || app_lower.contains("cursor")
        || app_lower.contains("idea")
        || app_lower.contains("pycharm")
        || app_lower.contains("webstorm")
        || app_lower.contains("goland")
        || app_lower.contains("clion")
        || app_lower.contains("rustrover")
        || app_lower.contains("rider")
        || app_lower.contains("phpstorm")
        || app_lower.contains("datagrip")
        || app_lower.contains("fleet")
        || app_lower.contains("xcode")
        || app_lower.contains("android studio")
        || app_lower.contains("hbuilder")
        || app_lower.contains("sublime")
        || app_lower.contains("atom")
        || app_lower.contains("vim")
        || app_lower.contains("neovim")
        || app_lower.contains("emacs")
        || app_lower.contains("nova")
        || app_lower.contains("bbedit")
        || app_lower.contains("coteditor")
        || app_lower.contains("textmate")
        || app_lower.contains("terminal")
        || app_lower.contains("iterm")
        || app_lower.contains("warp")
        || app_lower.contains("alacritty")
        || app_lower.contains("kitty")
        || app_lower.contains("wezterm")
        || app_lower.contains("hyper")
        || app_lower.contains("windowsterminal")
        || app_lower.contains("cmd")
        || app_lower.contains("powershell")
        || app_lower.contains("git")
        || app_lower.contains("sourcetree")
        || app_lower.contains("gitkraken")
        || app_lower.contains("docker")
        || app_lower.contains("postman")
        || app_lower.contains("insomnia")
        || app_lower.contains("dbeaver")
        || app_lower.contains("navicat")
        || app_lower.contains("tableplus")
        || app_lower.contains("sequel")
        || app_lower.contains("charles")
        || app_lower.contains("fiddler")
    {
        return "development".to_string();
    }

    // 浏览器（支持市面上所有主流浏览器，包含 Windows 进程名）
    // 注意：短名称用精确匹配或 starts_with，避免误匹配系统进程
    if app_lower.contains("chrome")
        || app_lower.contains("firefox")
        || app_lower.contains("safari")
        || app_lower.contains("msedge")
        || app_lower.contains("microsoft edge")
        || app_lower.contains("opera")
        || app_lower.contains("brave")
        || app_lower.starts_with("arc")
        || app_lower.contains("vivaldi")
        || app_lower.contains("chromium")
        || app_lower.contains("orion")
        || app_lower.starts_with("zen")
        || app_lower.contains("sidekick")
        || app_lower.contains("wavebox")
        || app_lower.contains("maxthon")
        || app_lower.contains("waterfox")
        || app_lower.contains("librewolf")
        || app_lower.contains("tor browser")
        || app_lower.contains("duckduckgo")
        || app_lower.contains("yandex")
        || app_lower.starts_with("whale")
        || app_lower.contains("naver")
        || app_lower.contains("uc browser")
        || app_lower.contains("qq browser")
        || app_lower.contains("360 browser")
        || app_lower.contains("sogou browser")
        || app_lower.contains("qqbrowser")
        || app_lower.contains("360se")
        || app_lower.contains("360chrome")
        || app_lower.contains("sogouexplorer")
        || app_lower.contains("2345explorer")
        || app_lower.contains("liebao")
        || app_lower.contains("theworld")
        || app_lower.contains("centbrowser")
        || app_lower.contains("iexplore")
        || app_lower.contains("qq浏览器")
        || app_lower.contains("360浏览器")
        || app_lower.contains("搜狗浏览器")
    {
        return "browser".to_string();
    }

    // 通讯工具（注意：qq 的匹配要排除已被浏览器捕获的 qqbrowser）
    if app_lower.contains("slack")
        || app_lower.contains("teams")
        || app_lower.contains("zoom")
        || app_lower.contains("discord")
        || app_lower.contains("wechat")
        || app_lower.contains("微信")
        || app_lower.contains("wecom")
        || app_lower.contains("企业微信")
        || (app_lower.contains("qq") && !app_lower.contains("qqbrowser"))
        || app_lower.contains("telegram")
        || app_lower.contains("skype")
        || app_lower.contains("dingtalk")
        || app_lower.contains("钉钉")
        || app_lower.contains("飞书")
        || app_lower.contains("lark")
    {
        return "communication".to_string();
    }

    // 办公软件
    if app_lower.contains("word")
        || app_lower.contains("excel")
        || app_lower.contains("powerpoint")
        || app_lower.contains("pages")
        || app_lower.contains("numbers")
        || app_lower.contains("keynote")
        || app_lower.contains("notion")
        || app_lower.contains("obsidian")
        || app_lower.contains("logseq")
        || app_lower.contains("evernote")
        || app_lower.contains("onenote")
        || app_lower.contains("wps")
        || app_lower.contains("typora")
        || app_lower.contains("bear")
        || app_lower.contains("ulysses")
        || app_lower.contains("xmind")
        || app_lower.contains("mindnode")
    {
        return "office".to_string();
    }

    // 设计工具
    if app_lower.contains("figma")
        || app_lower.contains("sketch")
        || app_lower.contains("photoshop")
        || app_lower.contains("illustrator")
        || app_lower.contains("xd")
        || app_lower.contains("canva")
        || app_lower.contains("pixelmator")
        || app_lower.contains("affinity")
        || app_lower.contains("lightroom")
        || app_lower.contains("indesign")
    {
        return "design".to_string();
    }

    // 娱乐
    if app_lower.contains("spotify")
        || app_lower.contains("music")
        || app_lower.contains("youtube")
        || app_lower.contains("netflix")
        || app_lower.contains("bilibili")
        || app_lower.contains("game")
        || app_lower.contains("steam")
        || app_lower.contains("网易云")
        || app_lower.contains("qqmusic")
        || app_lower.contains("爱奇艺")
    {
        return "entertainment".to_string();
    }

    // 窗口标题兜底：app_name 无法识别时，用窗口标题中的 IDE/工具关键词做最后一轮匹配
    // 典型场景：Windows 上 JetBrains IDE 进程名可能是 java.exe / idea64.exe 截断后不匹配
    if !window_title.is_empty() {
        let title_lower = window_title.to_lowercase();
        if title_lower.contains("intellij")
            || title_lower.contains("pycharm")
            || title_lower.contains("webstorm")
            || title_lower.contains("goland")
            || title_lower.contains("clion")
            || title_lower.contains("datagrip")
            || title_lower.contains("rustrover")
            || title_lower.contains("visual studio")
            || title_lower.contains("vs code")
            || title_lower.contains("cursor")
        {
            return "development".to_string();
        }
    }

    "other".to_string()
}

pub fn find_category_override(
    rules: &[crate::config::AppCategoryRule],
    app_name: &str,
    custom_categories: &[crate::config::CustomCategory],
) -> Option<String> {
    let normalized_app_name = normalized_app_rule_key(app_name);
    let custom_keys: Vec<String> = custom_categories.iter().map(|c| c.key.clone()).collect();

    rules.iter().find_map(|rule| {
        let normalized_rule = normalized_app_rule_key(&rule.app_name);
        if normalized_app_name == normalized_rule
            || normalized_app_name.contains(&normalized_rule)
            || normalized_rule.contains(&normalized_app_name)
        {
            Some(crate::config::normalize_category_key_private(
                &rule.category,
                &custom_keys,
            ))
        } else {
            None
        }
    })
}

pub fn categorize_app_with_rules(
    rules: &[crate::config::AppCategoryRule],
    app_name: &str,
    window_title: &str,
    custom_categories: &[crate::config::CustomCategory],
) -> String {
    find_category_override(rules, app_name, custom_categories)
        .unwrap_or_else(|| categorize_app(app_name, window_title))
}


pub fn resolve_activity_classification(
    config: &AppConfig,
    app_name: &str,
    window_title: &str,
    browser_url: Option<&str>,
) -> activity_classifier::ActivityClassification {
    let mut base_category = categorize_app_with_rules(
        &config.app_category_rules,
        app_name,
        window_title,
        &config.custom_categories,
    );
    // 分类被删除时回退到 "other"
    if base_category != "other"
        && !config.custom_categories.iter().any(|c| c.key == base_category)
    {
        base_category = "other".to_string();
    }
    let classification = activity_classifier::classify_activity_with_base_category(
        app_name,
        window_title,
        browser_url,
        &base_category,
    );

    // 语义分类被删除时回退到 "未知活动"
    // if classification.semantic_category != "未知活动"
    //     && !config
    //         .custom_semantic_categories
    //         .iter()
    //         .any(|c| c.key == classification.semantic_category)
    // {
    //     classification.semantic_category = "未知活动".to_string();
    // }

    // if let Some(semantic_category) =
    //     monitor::find_website_semantic_override(&config.website_semantic_rules, browser_url)
    // {
    //     classification.base_category = monitor::semantic_category_to_base_category(
    //         &semantic_category,
    //         &classification.base_category,
    //     );
    //     classification.semantic_category = semantic_category.clone();
    //     classification.confidence = classification.confidence.max(100);
    //     classification
    //         .evidence
    //         .push(format!("命中网站语义规则: {semantic_category}"));
    // }

    classification
}

/// 应用自定义描述查找结果
#[derive(Debug, Clone, Default)]
pub struct AppDescriptionLookup {
    /// 自定义显示名称
    pub display_name: Option<String>,
    /// 自定义描述
    pub description: Option<String>,
    /// 自定义分类
    pub category: Option<String>,
}

/// 查找应用的自定义描述规则
/// 优先匹配用户自定义规则，未匹配时回退到内置描述
pub fn find_app_description(
    rules: &[crate::config::AppDescriptionRule],
    app_name: &str,
) -> Option<AppDescriptionLookup> {
    let normalized_app_name = normalized_app_rule_key(app_name);

    // 优先查找用户自定义规则
    let user_result = rules.iter().find_map(|rule| {
        let normalized_rule = normalized_app_rule_key(&rule.app_name);
        if normalized_app_name == normalized_rule
            || normalized_app_name.contains(&normalized_rule)
            || normalized_rule.contains(&normalized_app_name)
        {
            Some(AppDescriptionLookup {
                display_name: rule.display_name.clone(),
                description: rule.description.clone(),
                category: rule.category.clone(),
            })
        } else {
            None
        }
    });

    if user_result.is_some() {
        return user_result;
    }

    // 回退到内置描述
    builtin_app_description(&normalized_app_name)
}

/// 内置常见应用描述（主要覆盖托盘程序）
fn builtin_app_description(normalized_name: &str) -> Option<AppDescriptionLookup> {
    let description = match normalized_name {
        // ── 通讯 / 社交 ──
        "wechat" | "weixin" => "即时通讯",
        "wecom" | "wxwork" => "企业通讯与协作",
        "qq" => "即时通讯",
        "telegram" => "跨平台加密即时通讯",
        "slack" => "团队协作与沟通",
        "discord" => "语音/文字社区平台",
        "teams" | "msteams" | "ms-teams" => "团队协作与会议",
        "dingtalk" => "企业通讯与办公",
        "feishu" | "lark" => "企业协作与知识管理",
        "zoom" | "zoom.us" => "视频会议",
        "skype" | "skypeapp" => "语音/视频通话",
        "whatsapp" => "跨平台即时通讯",
        "signal" => "端到端加密通讯",
        "line" => "即时通讯",

        // ── 云存储 / 同步 ──
        "onedrive" => "Microsoft 云存储同步",
        "dropbox" => "云存储同步",
        "googledrivesync" | "googledrive" => "Google 云存储同步",
        "icloud" | "iclouddrive" => "Apple 云存储同步",
        "坚果云" | "nutstore" => "国产云存储同步",

        // ── 开发工具 ──
        "docker desktop" | "docker" => "容器化开发平台",
        "postman" => "API 开发与测试",
        "insomnia" => "API 设计与调试",
        "sourcetree" => "Git 图形化客户端",
        "githubdesktop" | "github desktop" => "GitHub 官方客户端",
        "gitkraken" => "Git 图形化客户端",
        "fork" => "Git 图形化客户端",
        "navicat" | "navicatpremium" => "数据库管理工具",
        "tableplus" => "数据库管理工具",

        // ── 效率 / 工具 ──
        "pot" | "pot-app" => "跨平台划词翻译",
        "snipaste" | "snipaste.exe" => "截图与贴图工具",
        "everything" => "极速文件搜索",
        "listary" => "文件快速搜索与启动",
        "utools" => "效率工具集",
        "quicker" => "效率手势与快捷启动",
        "alfred" => "macOS 效率启动器",
        "raycast" => "macOS 效率启动器",
        "flameshot" => "Linux 截图工具",

        // ── 网络 / 代理 ──
        "clash" | "clash for windows" | "clash-verge" | "clash-nyanpasu" => "网络代理工具",
        "v2ray" | "v2rayn" | "v2rayu" => "网络代理工具",
        "shadowsocks" | "shadowsocksr" => "网络代理工具",
        "trojan" | "trojan-qt5" => "网络代理工具",
        "proxifier" => "网络代理转发",
        "wireguard" => "VPN 隧道",
        "openvpn" => "VPN 客户端",

        // ── 安全 / 杀毒 ──
        "360sd" | "360se" | "360tray" => "360 安全",
        "qqpctray" | "qqpcmgr" => "腾讯电脑管家",
        "huorong" | "hipstray" => "火绒安全",
        "avp" | "avpui" | "kavfs" => "Kaspersky 安全",
        "norton" | "ns.exe" => "Norton 安全",

        // ── 输入法 ──
        "sogouinput" | "sgmain" | "sogou" | "sgtool" => "搜狗输入法",
        "baidupinyin" | "baiduinput" => "百度输入法",
        "微软拼音" | "chsiime" => "微软拼音输入法",
        "rime" | "weasel" | "squirrel" => "RIME 输入法",

        // ── 硬件 / 驱动 ──
        "rtkngui" | "rtkaud" | "realtek" => "Realtek 音频管理",
        "nvtray" | "nvidia" | "nvidia-smi" => "NVIDIA 显卡驱动",
        "amdrsserv" | "amd" => "AMD 显卡驱动",
        "logioptions" | "logioptionsplus" => "Logitech 外设管理",
        "synapse" | "razer" => "Razer 外设管理",
        "steelseriesengine" => "SteelSeries 外设管理",
        "icue" | "corsair" => "Corsair 外设管理",

        // ── 下载 / 传输 ──
        "thunder" | "thunderbolt" => "迅雷下载",
        "idm" | "idman" => "IDM 下载管理器",
        "motrix" => "开源下载工具",
        "localsend" => "局域网文件传输",
        "feem" => "局域网文件传输",

        // ── 笔记 / 知识 ──
        "obsidian" => "本地知识库与笔记",
        "notion" => "协作笔记与知识库",
        "flomo" | "flomoplus" => "轻量笔记与想法记录",
        "ticktick" | "dida365" => "滴答清单 · 待办与日程",
        "todoist" => "待办任务管理",
        "wunderlist" => "待办任务管理",

        // ── 音乐 / 媒体 ──
        "netease_cloudmusic" | "cloudmusic" => "网易云音乐",
        "qqmusic" | "qqmusicuniversal" => "QQ 音乐",
        "spotify" => "Spotify 音乐流媒体",
        "kugou" | "kgagent" => "酷狗音乐",
        "kuwo" => "酷我音乐",
        "apple music" | "music" => "Apple Music",

        // ── 远程 / 虚拟化 ──
        "teamviewer" => "远程桌面与协作",
        "anydesk" => "远程桌面",
        "todesk" | "tovnavive" => "远程桌面",
        "sunloginclient" | "sunlogin" => "向日葵远程控制",
        "rustdesk" => "开源远程桌面",
        "vmware" | "vmware-tray" => "VMware 虚拟机",
        "virtualbox" | "virtualboxvm" => "VirtualBox 虚拟机",
        "parallels" | "prl_cc" => "Parallels 虚拟机",

        // ── 浏览器 ──
        "msedge" | "microsoft edge" => "Microsoft 浏览器",
        "chrome" | "google chrome" => "Google 浏览器",
        "firefox" | "mozilla firefox" => "Mozilla 浏览器",
        "brave" => "Brave 浏览器",
        "opera" => "Opera 浏览器",
        "vivaldi" => "Vivaldi 浏览器",
        "arc" => "Arc 浏览器",
        "tor" | "tor browser" => "Tor 匿名浏览器",
        "360chrome" => "360 浏览器",
        "qqbrowser" => "QQ 浏览器",
        "sogouexplorer" | "sogou browser" => "搜狗浏览器",
        "maxthon" => "傲游浏览器",
        "liebao" | "kbrowser" => "猎豹浏览器",
        "ucbrowser" | "uc" => "UC 浏览器",
        "quark" => "夸克浏览器",

        _ => return None,
    };

    Some(AppDescriptionLookup {
        display_name: None,
        description: Some(description.to_string()),
        category: None,
    })
}