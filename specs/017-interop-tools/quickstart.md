# Quickstart: 017-interop-tools

## Prerequisites
- `iris-dev` built from branch `017-interop-tools`
- IRIS instance with Interoperability/Ensemble enabled (iris-dev-iris on localhost:52780)

## Check production status
```bash
iris-dev mcp  # then via MCP client:
# tools/call interop_production_status {}
```

## Start a production
```bash
# tools/call interop_production_start {"production": "Demo.TicketIngestion"}
```

## Monitor logs
```bash
# tools/call interop_logs {"limit": 20, "log_type": "error"}
```

## Check queues
```bash
# tools/call interop_queues {}
```

## Demo flow
1. `interop_production_status` → see current state
2. `interop_production_stop` → deliberately stall
3. `interop_logs` → see the error
4. `interop_production_start` → recover
5. `interop_queues` → verify no backlog

## Test commands
```bash
# Unit tests (no IRIS)
cargo test --test interop_unit_tests

# E2e tests (requires iris-dev-iris)
IRIS_HOST=localhost IRIS_WEB_PORT=52780 IRIS_USERNAME=_SYSTEM IRIS_PASSWORD=SYS cargo test --test interop_e2e_tests
```
