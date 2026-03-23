# MCP Tool Contracts: 017-interop-tools

## MVP Tools (9)

### interop_production_status
Input: `{"namespace": "USER", "full_status": false}`
Output: `{"success": true, "production": "name", "state": "Running|Stopped|Troubled|Suspended", "state_code": 1}`
With full_status: adds `"items": [...]` array

### interop_production_start
Input: `{"production": "Demo.TicketIngestion", "namespace": "USER"}`
Output: `{"success": true, "production": "Demo.TicketIngestion", "state": "Running"}`
Error: `{"success": false, "error_code": "INTEROP_ERROR", "error": "..."}`

### interop_production_stop
Input: `{"production": "Demo.TicketIngestion", "timeout": 30, "force": false}`
Output: `{"success": true, "state": "Stopped"}`

### interop_production_update
Input: `{"timeout": 30, "force": false}`
Output: `{"success": true, "message": "Production updated"}`

### interop_production_needs_update
Input: `{}`
Output: `{"needs_update": true|false}`

### interop_production_recover
Input: `{}`
Output: `{"success": true, "state": "Running"}`

### interop_logs
Input: `{"item_name": null, "limit": 10, "log_type": "error,warning"}`
Output: `{"success": true, "logs": [{"id":..., "timestamp":..., "type":..., "component":..., "text":...}], "count": N}`

### interop_queues
Input: `{}`
Output: `{"success": true, "queues": [{"name": "...", "count": N}]}`

### interop_message_search
Input: `{"source": null, "target": null, "class_name": null, "limit": 20}`
Output: `{"success": true, "messages": [{"id":..., "timestamp":..., "source":..., "target":..., "class":..., "status":...}], "count": N}`
