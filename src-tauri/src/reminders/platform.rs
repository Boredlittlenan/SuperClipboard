#[cfg(windows)]
mod windows {
    use std::io::Write;
    use std::os::windows::process::CommandExt;
    use std::path::Path;
    use std::process::{Command, Output};
    use std::sync::Mutex;

    static REGISTERED: Mutex<bool> = Mutex::new(false);
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    fn program(name: &str, args: &[&str]) -> Result<Output, String> {
        let system =
            std::env::var_os("SystemRoot").ok_or("Windows system directory is unavailable")?;
        Command::new(std::path::PathBuf::from(system).join("System32").join(name))
            .creation_flags(CREATE_NO_WINDOW)
            .args(args)
            .output()
            .map_err(|e| e.to_string())
    }

    fn escape_xml(input: &str) -> String {
        input
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
    }

    pub fn ensure_schedule(path: &Path) -> Result<(), String> {
        let mut registered = REGISTERED.lock().map_err(|e| e.to_string())?;
        if *registered {
            return Ok(());
        }
        let account = program("whoami.exe", &["/user", "/fo", "csv", "/nh"])?;
        let account = String::from_utf8_lossy(&account.stdout);
        let sid = account
            .trim()
            .rsplit(',')
            .next()
            .unwrap_or_default()
            .trim_matches('"');
        if !sid.starts_with("S-1-")
            || !sid
                .bytes()
                .all(|ch| ch.is_ascii_digit() || ch == b'S' || ch == b'-')
        {
            return Err("Cannot determine Windows user for reminder scheduling".into());
        }
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let boundary = chrono::Local::now().format("%Y-%m-%dT%H:%M:00");
        let task = if cfg!(debug_assertions) {
            "SuperClipboard 4.0 Development Reminders"
        } else {
            "SuperClipboard Reminders"
        };
        let name = format!("{task} ({sid})");
        let args = format!(
            "--reminder-tick &quot;{}&quot;",
            escape_xml(&path.to_string_lossy())
        );
        let xml = format!(
            r#"<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo><Description>Deliver SuperClipboard memo reminders for this Windows user.</Description></RegistrationInfo>
  <Triggers>
    <CalendarTrigger>
      <Repetition><Interval>PT1M</Interval><Duration>P1D</Duration><StopAtDurationEnd>false</StopAtDurationEnd></Repetition>
      <StartBoundary>{boundary}</StartBoundary><Enabled>true</Enabled>
      <ScheduleByDay><DaysInterval>1</DaysInterval></ScheduleByDay>
    </CalendarTrigger>
    <LogonTrigger><Enabled>true</Enabled><UserId>{sid}</UserId></LogonTrigger>
  </Triggers>
  <Principals><Principal id="CurrentUser"><UserId>{sid}</UserId><LogonType>InteractiveToken</LogonType><RunLevel>LeastPrivilege</RunLevel></Principal></Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <StartWhenAvailable>true</StartWhenAvailable>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>true</Enabled>
    <ExecutionTimeLimit>PT1M</ExecutionTimeLimit>
  </Settings>
  <Actions Context="CurrentUser"><Exec><Command>{}</Command><Arguments>{args}</Arguments></Exec></Actions>
</Task>"#,
            escape_xml(&exe.to_string_lossy()),
        );
        let mut file = tempfile::NamedTempFile::new().map_err(|e| e.to_string())?;
        file.write_all(xml.as_bytes()).map_err(|e| e.to_string())?;
        file.flush().map_err(|e| e.to_string())?;
        let xml_path = file.into_temp_path();
        let output = program(
            "schtasks.exe",
            &[
                "/Create",
                "/TN",
                &name,
                "/XML",
                &xml_path.to_string_lossy(),
                "/F",
                "/HRESULT",
            ],
        )?;
        if !output.status.success() {
            let detail = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "Windows Task Scheduler refused the reminder task ({}). {}",
                output.status,
                detail.trim()
            ));
        }
        *registered = true;
        Ok(())
    }

    pub fn notify(title: &str) -> Result<(), String> {
        let mut notification = notify_rust::Notification::new();
        notification
            .summary("SuperClipboard")
            .body(&title.chars().take(180).collect::<String>());
        if !cfg!(debug_assertions) {
            notification.app_id("com.superclipboard.app");
        }
        notification.show().map(|_| ()).map_err(|e| e.to_string())
    }
}

#[cfg(windows)]
pub use windows::{ensure_schedule, notify};

#[cfg(not(windows))]
pub fn ensure_schedule(_path: &std::path::Path) -> Result<(), String> {
    Err("Reminder scheduling requires Windows".into())
}

#[cfg(not(windows))]
pub fn notify(_title: &str) -> Result<(), String> {
    Err("Reminder notifications require Windows".into())
}
