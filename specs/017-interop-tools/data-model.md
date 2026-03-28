# Data Model: 017-interop-tools

## Entities

### ProductionStatus
```json
{
  "success": true,
  "production": "Demo.TicketIngestion",
  "state": "Running",
  "state_code": 1,
  "items": [
    {"name": "TicketIngest", "enabled": true, "status": "Running", "class": "EnsLib.File.PassthroughService"},
    {"name": "TicketProcess", "enabled": true, "status": "Running", "class": "Demo.TicketBPL"}
  ]
}
```

### LogEntry
```json
{
  "id": 12345,
  "timestamp": "2026-03-22 14:30:00",
  "type": "error",
  "component": "TicketIngest",
  "text": "Connection refused to /isc/tickets/"
}
```

### QueueInfo
```json
{
  "name": "TicketProcess",
  "count": 42
}
```

### MessageSearchResult
```json
{
  "id": 67890,
  "timestamp": "2026-03-22 14:29:55",
  "source": "TicketIngest",
  "target": "TicketProcess",
  "class": "Ens.StreamContainer",
  "status": "Completed"
}
```

## State Machine: Production Lifecycle
```
Stopped → [start] → Running
Running → [stop] → Stopped
Running → [error] → Troubled
Troubled → [recover] → Running
Running → [update] → Running (config reloaded)
```
