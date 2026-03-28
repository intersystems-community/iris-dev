use iris_dev_core::tools::interop::*;

fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap()
}

mod interop_production_status {
    use super::*;

    #[test]
    fn iris_unreachable_when_no_connection() {
        let r = rt().block_on(interop_production_status_impl(None, ProductionStatusParams { namespace: "USER".into(), full_status: false }));
        let result = r.unwrap();
        let text = result.content[0].raw.as_text().unwrap().text.clone();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["success"], false);
        assert_eq!(v["error_code"], "IRIS_UNREACHABLE");
    }
}

mod interop_production_start {
    use super::*;

    #[test]
    fn iris_unreachable() {
        let r = rt().block_on(interop_production_start_impl(None, ProductionNameParams { production: Some("Test".into()), namespace: "USER".into() }));
        let result = r.unwrap();
        let text = result.content[0].raw.as_text().unwrap().text.clone();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["error_code"], "IRIS_UNREACHABLE");
    }
}

mod interop_production_stop {
    use super::*;

    #[test]
    fn iris_unreachable() {
        let r = rt().block_on(interop_production_stop_impl(None, ProductionStopParams { production: None, namespace: "USER".into(), timeout: 30, force: false }));
        let result = r.unwrap();
        let text = result.content[0].raw.as_text().unwrap().text.clone();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["error_code"], "IRIS_UNREACHABLE");
    }
}

mod interop_production_update {
    use super::*;

    #[test]
    fn iris_unreachable() {
        let r = rt().block_on(interop_production_update_impl(None, ProductionUpdateParams { timeout: 30, force: false }));
        let result = r.unwrap();
        let text = result.content[0].raw.as_text().unwrap().text.clone();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["error_code"], "IRIS_UNREACHABLE");
    }
}

mod interop_production_needs_update {
    use super::*;

    #[test]
    fn iris_unreachable() {
        let r = rt().block_on(interop_production_needs_update_impl(None));
        let result = r.unwrap();
        let text = result.content[0].raw.as_text().unwrap().text.clone();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["error_code"], "IRIS_UNREACHABLE");
    }
}

mod interop_production_recover {
    use super::*;

    #[test]
    fn iris_unreachable() {
        let r = rt().block_on(interop_production_recover_impl(None));
        let result = r.unwrap();
        let text = result.content[0].raw.as_text().unwrap().text.clone();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["error_code"], "IRIS_UNREACHABLE");
    }
}

mod interop_logs {
    use super::*;

    #[test]
    fn iris_unreachable() {
        let r = rt().block_on(interop_logs_impl(None, LogsParams { item_name: None, limit: 10, log_type: "error".into() }));
        let result = r.unwrap();
        let text = result.content[0].raw.as_text().unwrap().text.clone();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["error_code"], "IRIS_UNREACHABLE");
    }
}

mod interop_queues {
    use super::*;

    #[test]
    fn iris_unreachable() {
        let r = rt().block_on(interop_queues_impl(None));
        let result = r.unwrap();
        let text = result.content[0].raw.as_text().unwrap().text.clone();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["error_code"], "IRIS_UNREACHABLE");
    }
}

mod interop_message_search {
    use super::*;

    #[test]
    fn iris_unreachable() {
        let r = rt().block_on(interop_message_search_impl(None, MessageSearchParams { source: None, target: None, class_name: None, limit: 20 }));
        let result = r.unwrap();
        let text = result.content[0].raw.as_text().unwrap().text.clone();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["error_code"], "IRIS_UNREACHABLE");
    }
}

mod parse_status {
    use iris_dev_core::tools::interop::parse_status_response;

    #[test]
    fn running() {
        let (name, code, state) = parse_status_response("Demo.Prod:1").unwrap();
        assert_eq!(name, "Demo.Prod");
        assert_eq!(code, 1);
        assert_eq!(state, "Running");
    }

    #[test]
    fn stopped() {
        let (_, code, state) = parse_status_response("Demo.Prod:2").unwrap();
        assert_eq!(code, 2);
        assert_eq!(state, "Stopped");
    }

    #[test]
    fn troubled() {
        let (_, code, state) = parse_status_response("Demo.Prod:4").unwrap();
        assert_eq!(state, "Troubled");
    }

    #[test]
    fn no_production() {
        assert!(parse_status_response(":").is_err());
        assert!(parse_status_response("").is_err());
    }

    #[test]
    fn interop_error() {
        let err = parse_status_response("ERROR:Something went wrong").unwrap_err();
        assert!(err.starts_with("INTEROP_ERROR"));
    }
}
