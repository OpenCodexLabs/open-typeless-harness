#[cfg(target_os = "macos")]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--wait") {
        if args.len() < 2 {
            anyhow::bail!("--wait requires seconds");
        }
        let wait_secs = args[1].parse::<u64>()?;
        args.drain(0..2);
        tokio::time::sleep(std::time::Duration::from_secs(wait_secs)).await;
    }
    let explicit_pid = if args.first().map(String::as_str) == Some("--pid") {
        if args.len() < 2 {
            anyhow::bail!("--pid requires a process id");
        }
        let pid = args[1].parse::<i32>()?;
        args.drain(0..2);
        Some(pid)
    } else {
        None
    };
    let mut args = args.into_iter();
    let original_text = args.next().unwrap_or_else(|| "probe raw text".to_string());
    let inserted_text = args
        .next()
        .unwrap_or_else(|| "probe inserted text".to_string());

    eprintln!("Monitoring current focused text field for 30s. Edit the field to produce JSONL.");
    match explicit_pid {
        Some(pid) => {
            openless_lib::edit_monitor::probe_target_pid(pid, original_text, inserted_text).await
        }
        None => openless_lib::edit_monitor::probe_current_focus(original_text, inserted_text).await,
    }
}

#[cfg(not(target_os = "macos"))]
fn main() -> anyhow::Result<()> {
    anyhow::bail!("edit_monitor_probe is only implemented on macOS")
}
