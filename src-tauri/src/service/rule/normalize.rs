use super::extract;
use super::determine;


pub fn normalized_windows_system_window_text(value: &str) -> String {
    value.trim().to_lowercase()
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn trim_url_candidate(value: &str) -> &str {
    value.trim().trim_matches(|c: char| {
        matches!(
            c,
            '"' | '\'' | '`' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>' | ',' | ';'
        )
    })
}


#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub fn normalize_possible_url(value: &str) -> Option<String> {
    let candidate = trim_url_candidate(value)
        .trim_matches(|c: char| c.is_control() || c == '\u{200b}' || c == '\u{feff}')
        .trim_end_matches('.');

    if candidate.is_empty() {
        return None;
    }

    if candidate.contains(' ') {
        return None;
    }

    let candidate_lower = candidate.to_lowercase();
    if candidate_lower.starts_with("http://") || candidate_lower.starts_with("https://") {
        return Some(candidate.to_string());
    }

    if candidate.contains("://")
        || candidate_lower.starts_with("about:")
        || candidate_lower.starts_with("chrome:")
        || candidate_lower.starts_with("edge:")
        || candidate_lower.starts_with("file:")
    {
        return Some(candidate.to_string());
    }

    let (host, _) = extract::split_host_and_rest(candidate);
    if determine::is_probable_host(host) {
        let result = format!(
            "{}{}",
            if extract::split_host_port(host).0.to_lowercase() == "localhost"
                || determine::is_probable_ipv4(extract::split_host_port(host).0)
            {
                "http://"
            } else {
                "https://"
            },
            candidate.trim_end_matches('/')
        );
        if determine::is_merged_domain(&result) {
            return None;
        }
        return Some(result);
    }

    if determine::is_probable_domain(candidate) {
        let result = format!("https://{}", candidate.trim_end_matches('/'));
        if determine::is_merged_domain(&result) {
            return None;
        }
        return Some(result);
    }

    None
}


pub fn normalize_session_store_title(value: &str) -> String {
    value
        .split(" - Mozilla Firefox")
        .next()
        .unwrap_or(value)
        .split(" - Firefox")
        .next()
        .unwrap_or(value)
        .split(" - Zen Browser")
        .next()
        .unwrap_or(value)
        .split(" - Zen")
        .next()
        .unwrap_or(value)
        .trim()
        .to_string()
}


/// 统一应用显示名称，避免不同来源（进程名、数据库历史、运行中列表）出现重复项
pub fn normalize_display_app_name(app_name: &str) -> String {
    let trimmed = app_name
        .trim()
        .trim_end_matches(".exe")
        .trim_end_matches(".EXE")
        .trim();

    let normalized = trimmed.to_lowercase();
    let compact = normalized
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect::<String>();

    if (normalized.contains("work_review")
        || normalized.contains("work-review")
        || normalized.contains("work review")
        || compact.contains("workreview"))
        && (normalized.contains("setup")
            || normalized.contains("installer")
            || compact.contains("setup")
            || compact.contains("installer"))
    {
        return "Work Review Setup".to_string();
    }

    match normalized.as_str() {
        // ── 本应用 ──
        "work-review" | "work_review" | "workreview" | "work review" => "Work Review".to_string(),
        // ── 浏览器 ──
        "chrome" | "google chrome" => "Google Chrome".to_string(),
        "msedge" | "edge" | "microsoft edge" => "Microsoft Edge".to_string(),
        "brave" | "brave browser" => "Brave Browser".to_string(),
        "firefox" => "Firefox".to_string(),
        "safari" => "Safari".to_string(),
        "opera" => "Opera".to_string(),
        "vivaldi" => "Vivaldi".to_string(),
        "chromium" => "Chromium".to_string(),
        "arc" => "Arc".to_string(),
        "zen browser" | "zen" => "Zen Browser".to_string(),
        "qqbrowser" | "qq browser" | "qq浏览器" => "QQ Browser".to_string(),
        "360se" | "360chrome" | "360 browser" | "360浏览器" => "360 Browser".to_string(),
        "sogouexplorer" | "sogou browser" | "搜狗浏览器" => "Sogou Browser".to_string(),
        "maxthon" => "Maxthon".to_string(),
        "yandex" | "yandex browser" => "Yandex Browser".to_string(),
        "tor" | "tor browser" => "Tor Browser".to_string(),
        "waterfox" => "Waterfox".to_string(),
        "librewolf" => "LibreWolf".to_string(),
        "floorp" => "Floorp".to_string(),
        "iceweasel" => "Iceweasel".to_string(),
        // ── IDE / 编辑器 ──
        "code" | "vscode" | "visual studio code" | "vs code" => "VS Code".to_string(),
        "cursor" => "Cursor".to_string(),
        "windsurf" | "antigravity" => "Windsurf".to_string(),
        "idea" | "idea64" | "intellij idea" => "IntelliJ IDEA".to_string(),
        "pycharm" | "pycharm64" => "PyCharm".to_string(),
        "webstorm" | "webstorm64" => "WebStorm".to_string(),
        "goland" | "goland64" => "GoLand".to_string(),
        "clion" | "clion64" => "CLion".to_string(),
        "rider" | "rider64" => "Rider".to_string(),
        "phpstorm" | "phpstorm64" => "PhpStorm".to_string(),
        "rubymine" | "rubymine64" => "RubyMine".to_string(),
        "datagrip" | "datagrip64" => "DataGrip".to_string(),
        "fleet" => "Fleet".to_string(),
        "android studio" | "studio64" => "Android Studio".to_string(),
        "devenv" | "visual studio" => "Visual Studio".to_string(),
        "xcode" => "Xcode".to_string(),
        "sublime_text" | "sublime text" => "Sublime Text".to_string(),
        "atom" => "Atom".to_string(),
        "zed" | "zed-editor" => "Zed".to_string(),
        "nova" => "Nova".to_string(),
        "textmate" => "TextMate".to_string(),
        "vim" | "gvim" | "mvim" => "Vim".to_string(),
        "nvim" => "Neovim".to_string(),
        "emacs" => "Emacs".to_string(),
        "codeblocks" => "Code::Blocks".to_string(),
        // ── 通讯 / 社交 ──
        "wechat" | "weixin" | "微信" => "WeChat".to_string(),
        "wecom" | "企业微信" | "wxwork" => "WeCom".to_string(),
        "qq" => "QQ".to_string(),
        "telegram" | "telegram desktop" => "Telegram".to_string(),
        "slack" => "Slack".to_string(),
        "discord" => "Discord".to_string(),
        "teams" | "msteams" | "ms-teams" | "microsoft teams" => "Microsoft Teams".to_string(),
        "dingtalk" | "钉钉" => "DingTalk".to_string(),
        "feishu" | "飞书" | "lark" => "Feishu".to_string(),
        "zoom" | "zoom.us" => "Zoom".to_string(),
        "skype" | "skypeapp" => "Skype".to_string(),
        "line" => "LINE".to_string(),
        "whatsapp" => "WhatsApp".to_string(),
        "signal" => "Signal".to_string(),
        "steam" | "steamwebhelper" => "Steam".to_string(),
        // ── 办公 / 笔记 ──
        "notion" | "notion-enhanced" | "notion-enhanced-app" => "Notion".to_string(),
        "obsidian" => "Obsidian".to_string(),
        "typora" => "Typora".to_string(),
        "marktext" | "mark text" => "Mark Text".to_string(),
        "onenote" | "microsoft onenote" => "OneNote".to_string(),
        "evernote" => "Evernote".to_string(),
        "youdaonote" | "有道云笔记" => "Youdao Note".to_string(),
        "yuque" | "语雀" => "Yuque".to_string(),
        // ── Microsoft Office ──
        "winword" | "word" => "Microsoft Word".to_string(),
        "excel" => "Microsoft Excel".to_string(),
        "powerpnt" | "powerpoint" => "Microsoft PowerPoint".to_string(),
        "outlook" => "Microsoft Outlook".to_string(),
        "msaccess" | "access" => "Microsoft Access".to_string(),
        "mspub" | "publisher" => "Microsoft Publisher".to_string(),
        "et" | "wps" => "WPS Office".to_string(),
        "wpp" => "WPS Presentation".to_string(),
        "wpspdf" => "WPS PDF".to_string(),
        // ── 终端 ──
        "windowsterminal" | "windows terminal" | "windowsterminal.exe" => "Windows Terminal".to_string(),
        "powershell" | "pwsh" => "PowerShell".to_string(),
        "cmd" => "Command Prompt".to_string(),
        "iterm2" | "iterm" => "iTerm2".to_string(),
        "terminal" | "terminal.app" => "Terminal".to_string(),
        "warp" => "Warp".to_string(),
        "alacritty" => "Alacritty".to_string(),
        "kitty" => "Kitty".to_string(),
        "wezterm" | "wezterm-gui" => "WezTerm".to_string(),
        "hyper" => "Hyper".to_string(),
        "tabby" => "Tabby".to_string(),
        "terminus" => "Terminus".to_string(),
        "mobaxterm" | "mobaxterm1" => "MobaXterm".to_string(),
        "putty" => "PuTTY".to_string(),
        // Linux 终端
        "gnome-terminal" | "gnome-terminal-server" => "GNOME Terminal".to_string(),
        "xfce4-terminal" => "Xfce Terminal".to_string(),
        "konsole" => "Konsole".to_string(),
        "tilix" => "Tilix".to_string(),
        "terminator" => "Terminator".to_string(),
        // ── 文件管理器 ──
        "explorer" => "File Explorer".to_string(),
        "finder" => "Finder".to_string(),
        "nemo" => "Nemo".to_string(),
        "nautilus" | "org.gnome.nautilus" => "Files".to_string(),
        "thunar" => "Thunar".to_string(),
        "dolphin" => "Dolphin".to_string(),
        // ── 设计 / 绘图 ──
        "figma" => "Figma".to_string(),
        "xd" | "adobe xd" => "Adobe XD".to_string(),
        "photoshop" | "adobe photoshop" => "Photoshop".to_string(),
        "illustrator" | "adobe illustrator" => "Illustrator".to_string(),
        "sketch" => "Sketch".to_string(),
        "inkscape" => "Inkscape".to_string(),
        "gimp" => "GIMP".to_string(),
        "blender" => "Blender".to_string(),
        "canva" => "Canva".to_string(),
        // ── 音乐 / 视频 ──
        "spotify" => "Spotify".to_string(),
        "netease_cloudmusic" | "cloudmusic" | "网易云音乐" => "NetEase Cloud Music".to_string(),
        "qqmusic" | "qqmusicuniversal" | "qq音乐" => "QQ Music".to_string(),
        "kugou" | "酷狗音乐" => "KuGou Music".to_string(),
        "kuwo" | "酷我音乐" => "KuWo Music".to_string(),
        "vlc" => "VLC".to_string(),
        "potplayer" | "potplayermini64" => "PotPlayer".to_string(),
        "mpv" => "mpv".to_string(),
        "iina" => "IINA".to_string(),
        "apple music" | "music" => "Music".to_string(),
        // ── 开发工具 ──
        "docker desktop" => "Docker Desktop".to_string(),
        "postman" => "Postman".to_string(),
        "insomnia" => "Insomnia".to_string(),
        "fork" | "fork-git-client" => "Fork".to_string(),
        "sourcetree" => "SourceTree".to_string(),
        "githubdesktop" | "github desktop" => "GitHub Desktop".to_string(),
        "gitkraken" => "GitKraken".to_string(),
        "tableplus" => "TablePlus".to_string(),
        "navicat" | "navicatpremium" => "Navicat".to_string(),
        "robomongo" | "robo3t" | "studio 3t" => "MongoDB Compass".to_string(),
        "redis-desktop-manager" | "rdm" => "RedisInsight".to_string(),
        // ── 远程桌面 / SSH ──
        "mstsc" => "Remote Desktop".to_string(),
        "teamviewer" => "TeamViewer".to_string(),
        "anydesk" => "AnyDesk".to_string(),
        "tovnavive" | "to desk" => "ToDesk".to_string(),
        "sunloginclient" | "sunlogin" => "Sunlogin".to_string(),
        // ── 其他 ──
        "mail" | "apple mail" | "邮件" => "Mail".to_string(),
        "discover" | "org.kde.discover" => "Discover".to_string(),
        "coreautha" | "coreauthuiagent" | "coreauthenticationuiagent" => {
            "System Authentication".to_string()
        }
        "xfltd" => "XFLTD".to_string(),
        "thunderbird" | "thunderbird-bin" => "Thunderbird".to_string(),
        "libreoffice" => "LibreOffice".to_string(),
        "evince" | "org.gnome.evince" => "Evince".to_string(),
        "eog" | "org.gnome.eog" => "Eye of GNOME".to_string(),
        "gedit" | "org.gnome.gedit" => "gedit".to_string(),
        "calibre" | "calibre-gui" => "Calibre".to_string(),
        // ── 系统工具 ──
        "lemon" | "tencent lemon" => "Tencent Lemon".to_string(),
        "cleanmymac" | "clean my mac" => "CleanMyMac".to_string(),
        "alfred" => "Alfred".to_string(),
        "raycast" => "Raycast".to_string(),
        "bartender" => "Bartender".to_string(),
        "istat menus" | "istat" => "iStat Menus".to_string(),
        "appcleaner" | "app cleaner" => "AppCleaner".to_string(),
        "the unarchiver" | "unarchiver" => "The Unarchiver".to_string(),
        "keka" => "Keka".to_string(),
        "daisydisk" => "DaisyDisk".to_string(),
        "onyx" => "OnyX".to_string(),
        "macpaw" => "MacPaw".to_string(),
        "sensei" => "Sensei".to_string(),
        "peak" => "Peak".to_string(),
        "ninjaclean" | "ninja clean" => "Ninja Clean".to_string(),
        "applink" => "AppLink".to_string(),
        "eqmac" => "eqMac".to_string(),
        "rectangle" => "Rectangle".to_string(),
        "magnet" => "Magnet".to_string(),
        "spectacle" => "Spectacle".to_string(),
        "amethyst" => "Amethyst".to_string(),
        "yabai" => "yabai".to_string(),
        "stats" => "Stats".to_string(),
        "monitor" => "Monitor".to_string(),
        _ => trimmed.to_string(),
    }
}



pub fn normalized_app_rule_key(app_name: &str) -> String {
    normalize_display_app_name(app_name).to_lowercase()
}