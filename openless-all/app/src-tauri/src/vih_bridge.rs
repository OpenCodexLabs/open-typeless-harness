use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use serde_json::Value;
use tokio::process::Command;

const DEFAULT_VIH_ROOT: &str = "/Users/lichenxin/proj/hub_edison/opentypeless/vih";
const DEFAULT_TIMEOUT_SECS: u64 = 12;

#[derive(Debug, Clone)]
pub struct VihRewrite {
    pub rewritten_text: String,
    pub route: String,
}

pub async fn rewrite(raw_text: &str) -> Result<Option<VihRewrite>> {
    if raw_text.trim().is_empty() || !enabled() {
        return Ok(None);
    }

    let root = vih_root();
    if !root.join("pyproject.toml").exists() {
        return Ok(None);
    }

    let timeout_secs = std::env::var("OPENTYPELESS_VIH_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(DEFAULT_TIMEOUT_SECS);

    let output = tokio::time::timeout(
        Duration::from_secs(timeout_secs),
        Command::new("uv")
            .arg("run")
            .arg("vih")
            .arg("rewrite")
            .arg(raw_text)
            .arg("--json")
            .current_dir(&root)
            .output(),
    )
    .await
    .context("VIH rewrite timed out")?
    .context("failed to spawn VIH rewrite")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!(
            "VIH rewrite exited with {}: {}",
            output.status,
            stderr.trim()
        );
    }

    let stdout = String::from_utf8(output.stdout).context("VIH stdout is not UTF-8")?;
    let value: Value = serde_json::from_str(stdout.trim()).context("VIH stdout is not JSON")?;
    let rewritten_text = value
        .get("rewritten_text")
        .and_then(Value::as_str)
        .unwrap_or(raw_text)
        .trim()
        .to_string();

    if rewritten_text.is_empty() {
        return Ok(None);
    }

    let route = value
        .get("route")
        .and_then(Value::as_str)
        .unwrap_or("output_text")
        .to_string();

    Ok(Some(VihRewrite {
        rewritten_text,
        route,
    }))
}

fn enabled() -> bool {
    std::env::var("OPENTYPELESS_VIH_ENABLED")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn vih_root() -> PathBuf {
    std::env::var_os("OPENTYPELESS_VIH_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_VIH_ROOT))
}
