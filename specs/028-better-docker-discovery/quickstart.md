# Quickstart: Testing Better Docker Discovery

## Running the regression harness

### Community images (no license key required)

```bash
cd ~/ws/iris-dev
IRIS_HOST=localhost cargo test --test docker_discovery_e2e
```

Spins up fresh `iris-community:2026.1` and `irishealth-community:2026.1` containers,
verifies each failure mode produces the correct message.

### Enterprise images (license key required)

```bash
IRIS_LICENSE_KEY_PATH=~/license/iris.key \
  cargo test --test docker_discovery_e2e -- --ignored
```

Adds `iris:2026.1` and `irishealth:2026.1` — verifies the web-server-absent message.

---

## What each error now looks like

### Container not found
```
WARN iris_dev_core::iris::discovery: Container 'my-iris' not found in Docker — is it running? ('docker ps' to check)
```

### Port not mapped
```
WARN iris_dev_core::iris::discovery: Container 'my-iris' found but port 52773 is not mapped to a host port. Restart with: docker run -p <host_port>:52773 ...
Note: iris_execute and iris_test still work via docker exec.
```

### Atelier not responding (enterprise image)
```
WARN iris_dev_core::iris::discovery: Container 'my-enterprise' found at localhost:52791 but Atelier REST API is not responding.
Enterprise IRIS images (iris:, irishealth:) do not include the private web server — use iris-community or irishealth-community for local dev, or connect via IRIS_HOST+IRIS_WEB_PORT to an external Web Gateway.
Note: iris_execute and iris_test still work via docker exec.
```

### Auth failure (community image without IRIS_PASSWORD)
```
WARN iris_dev_core::iris::discovery: IRIS at localhost:52790 returned 401 — container 'my-community' may need IRIS_PASSWORD. Restart with: docker run -e IRIS_PASSWORD=SYS ...
```
*(Only one message — no second generic WARN)*

---

## Verifying the fix manually

```bash
# Should produce the new "not found in Docker" message (not "not reachable"):
IRIS_CONTAINER=nonexistent iris-dev mcp

# Should produce the "port not mapped" message:
docker run -d --name test-nomapped intersystemsdc/iris-community:latest --check-caps false
IRIS_CONTAINER=test-nomapped iris-dev mcp

# Should produce the enterprise "no private web server" message:
docker run -d --name test-enterprise -p 52799:52773 \
  -v ~/license/iris.key:/usr/irissys/mgr/iris.key:ro \
  containers.intersystems.com/intersystems/iris:2026.1 --check-caps false
IRIS_CONTAINER=test-enterprise iris-dev mcp

# Cleanup
docker rm -f test-nomapped test-enterprise
```
