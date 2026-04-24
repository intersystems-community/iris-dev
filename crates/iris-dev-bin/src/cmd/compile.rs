use anyhow::{Context, Result};
use clap::Args;
use iris_dev_core::iris::{
    connection::{DiscoverySource, IrisConnection},
    discovery::discover_iris,
};

#[derive(Args)]
pub struct CompileCommand {
    pub target: Option<String>,
    #[arg(long, env = "IRIS_HOST")]
    pub host: Option<String>,
    #[arg(long, env = "IRIS_WEB_PORT", default_value = "52773")]
    pub web_port: u16,
    #[arg(long, env = "IRIS_NAMESPACE", default_value = "USER")]
    pub namespace: String,
    #[arg(long, env = "IRIS_USERNAME")]
    pub username: Option<String>,
    #[arg(long, env = "IRIS_PASSWORD")]
    pub password: Option<String>,
    #[arg(long, default_value = "cuk")]
    pub flags: String,
    #[arg(long)]
    pub force_writable: bool,
    #[arg(long, default_value = "text")]
    pub format: String,
}

impl CompileCommand {
    pub async fn run(self) -> Result<()> {
        let explicit = self.host.as_ref().map(|host| {
            let base_url = format!("http://{}:{}", host, self.web_port);
            let username = self.username.as_deref().unwrap_or("_SYSTEM");
            let password = self.password.as_deref().unwrap_or("SYS");
            IrisConnection::new(
                base_url,
                &self.namespace,
                username,
                password,
                DiscoverySource::ExplicitFlag,
            )
        });

        let iris = discover_iris(explicit).await?.context(
            "No IRIS connection found — set IRIS_HOST or run iris-dev mcp for auto-discovery",
        )?;

        let client = IrisConnection::http_client()?;
        let target = self.target.as_deref().unwrap_or(".");

        let code = if target == "." {
            // Bug 1: CompileAll takes flags, not namespace. The namespace is selected by execute().
            format!(
                "Set sc=$SYSTEM.OBJ.CompileAll(\"{}\") If $System.Status.IsOK(sc) {{Write \"OK\"}} Else {{Write $System.Status.GetErrorText(sc)}}",
                self.flags
            )
        } else if target.ends_with(".cls") {
            let cls_text =
                std::fs::read_to_string(target).with_context(|| format!("reading {}", target))?;
            // Bug 2: derive class name from the "Class ..." declaration inside the file,
            // not from the file path (which would strip package components).
            let cls_name = cls_text
                .lines()
                .find(|l| l.trim_start().starts_with("Class "))
                .and_then(|l| l.split_whitespace().nth(1))
                .map(|s| s.to_string())
                .unwrap_or_else(|| {
                    // Fallback: convert path separators to dots and strip extension
                    target
                        .trim_end_matches(".cls")
                        .replace(['/', '\\'], ".")
                        .trim_start_matches('.')
                        .to_string()
                });
            let cls_text_crlf = cls_text
                .replace("\r\n", "\n")
                .replace('\r', "\n")
                .replace('\n', "\r\n");
            // FR-017/Mo3: use parameterized placeholders for both cls_name and cls_text.
            let set_result = iris.query(
                "SELECT $SYSTEM.Status.IsOK(##class(%Compiler.UDL.TextServices).SetTextFromString(NULL,?,?))",
                vec![
                    serde_json::Value::String(cls_name.clone()),
                    serde_json::Value::String(cls_text_crlf),
                ],
                &self.namespace,
                &client,
            ).await;
            match set_result {
                Ok(_) => format!(
                    "Set sc=$SYSTEM.OBJ.Compile(\"{}\",\"{}\") If $System.Status.IsOK(sc) {{Write \"OK\"}} Else {{Write $System.Status.GetErrorText(sc)}}",
                    cls_name, self.flags
                ),
                Err(e) => {
                    let result = serde_json::json!({"success": false, "error_code": "IRIS_COMPILE_FAILED", "error": e.to_string(), "target": target});
                    output_result(&result, &self.format);
                    std::process::exit(1);
                }
            }
        } else {
            format!(
                "Set sc=$SYSTEM.OBJ.Compile(\"{}\",\"{}\") If $System.Status.IsOK(sc) {{Write \"OK\"}} Else {{Write $System.Status.GetErrorText(sc)}}",
                target, self.flags
            )
        };

        // IDEV-1: try HTTP execution first (no IRIS_CONTAINER required).
        // Fall back to docker exec only if IRIS_CONTAINER is set to a non-empty value.
        let exec_result = match iris
            .execute_via_generator(&code, &self.namespace, &client)
            .await
        {
            Ok(out) => Ok(out),
            Err(_)
                if std::env::var("IRIS_CONTAINER")
                    .ok()
                    .filter(|v| !v.is_empty())
                    .is_some() =>
            {
                iris.execute(&code, &self.namespace).await
            }
            Err(e) => Err(e),
        };
        match exec_result {
            Ok(out) => {
                let out = out.trim().to_string();
                if out == "OK" {
                    let result = serde_json::json!({"success": true, "target": target, "namespace": self.namespace, "stdout": "Compiled successfully"});
                    output_result(&result, &self.format);
                    Ok(())
                } else {
                    let result = serde_json::json!({"success": false, "error_code": "IRIS_COMPILE_FAILED", "error": out, "target": target});
                    output_result(&result, &self.format);
                    std::process::exit(1);
                }
            }
            Err(e) => {
                let msg = e.to_string();
                let ec = if msg == "DOCKER_REQUIRED" {
                    "DOCKER_REQUIRED"
                } else {
                    "IRIS_UNREACHABLE"
                };
                let result = serde_json::json!({"success": false, "error_code": ec, "error": msg});
                output_result(&result, &self.format);
                std::process::exit(2);
            }
        }
    }
}

fn output_result(result: &serde_json::Value, format: &str) {
    if format == "json" {
        println!("{}", result);
    } else if result["success"] == true {
        println!("✓ Compiled: {}", result["target"].as_str().unwrap_or(""));
    } else {
        eprintln!(
            "✗ Error [{}]: {}",
            result["error_code"].as_str().unwrap_or(""),
            result["error"].as_str().unwrap_or("")
        );
    }
}
