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
            format!(
                "Set sc=$SYSTEM.OBJ.CompileAll(\"{}\") If $System.Status.IsOK(sc) {{Write \"OK\"}} Else {{Write $System.Status.GetErrorText(sc)}}",
                self.namespace
            )
        } else if target.ends_with(".cls") {
            let cls_name = std::path::Path::new(target)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(target);
            let cls_text =
                std::fs::read_to_string(target).with_context(|| format!("reading {}", target))?;
            let cls_text_crlf = cls_text
                .replace("\r\n", "\n")
                .replace('\r', "\n")
                .replace('\n', "\r\n");
            let set_result = iris.query(
                &format!("SELECT $SYSTEM.Status.IsOK(##class(%Compiler.UDL.TextServices).SetTextFromString(NULL,'{}',?))", cls_name),
                vec![serde_json::Value::String(cls_text_crlf)],
                &client
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

        match iris.execute(&code, &self.namespace).await {
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
