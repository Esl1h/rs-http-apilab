# rs-http-apilab

Deterministic HTTP backend with 100% synthetic data that simulates an
internal corporate API. Run the binary and the port opens immediately:
no database, no migrations, no config files (restarting restores the
initial state).

This is the Rust port of the [v-http-apilab](https://github.com/Esl1h/v-http-apilab)
concept: same endpoints, same environment variables, same error
envelope, same proof-of-receipt buffer (`/lab/calls`) and the same curl
test suite, with the HTTP framework swapped from `veb` (V) to `axum`
(Rust).

Interactive documentation (Swagger UI) at `/docs` and the embedded
OpenAPI 3.0.3 specification at `/openapi.json`.

> **Warning**: lab tool. This is not a production template, it is not
> secure as a real application and must not be exposed to uncontrolled
> networks. Authentication is simulated, data is synthetic and nothing
> is persisted.

## Compiler and HTTP framework versions

| Item | Pinned value |
| --- | --- |
| Language | Rust, `rustc 1.98.1` (pinned by `rust-toolchain.toml`) |
| Async runtime | tokio 1.53.2 |
| HTTP framework | axum 0.8.9 (on hyper 1.12.0 + hyper-util 0.1.21) |
| Other dependencies | serde 1.0.229, serde_json 1.0.154, uuid 1.28.0, sha2 0.10.9, form_urlencoded 1.2.2, percent-encoding 2.3.2, tokio-stream 0.1.19, tower 0.5.3 |
| Locking | exact versions pinned by the committed `Cargo.lock`; builds use `--locked` |

Justifications:

1. `axum` is the maintained web framework on hyper 1.x/tokio with
   routing, native multipart and streaming response bodies (the
   v-http-apilab `/lab/stream` required a manual connection takeover;
   here streaming is native).
2. The server does not use `axum::serve` (which exposes no HTTP-level
   configuration): the accept loop is ours, with `header_read_timeout`
   (maps `READ_TIMEOUT_MS` with native millisecond granularity) and a
   16 KiB read buffer (the same header-size limit documented in
   v-http-apilab).
3. Routing is a private table behind a catch-all (the same architecture
   as v-http-apilab): every request, including 404/405 and exotic methods
   such as `TRACE`, gets the JSON envelope, the `Allow` header when
   applicable, a log line and a `/lab/calls` record.

## Build and run (local)

Requirements: Rust 1.98.1 (resolved via rustup from
`rust-toolchain.toml`), `curl`, `bash`.

```sh
./scripts/build.sh          # compiles bin/rs-http-apilab (bakes the rustc version into the binary)
./scripts/run.sh            # starts the server (PORT=8080 by default)
```

Examples:

```sh
PORT=9090 scripts/run.sh
LAB_ADMIN_TOKEN=my-token scripts/run.sh   # enables /lab/calls
READY_FORCE_FAIL=1 scripts/run.sh         # /ready starts answering 503
```

Live documentation:

- `http://localhost:8080/docs`: Swagger UI (UI assets come from a CDN;
  the API itself is self-contained).
- `http://localhost:8080/openapi.json`: OpenAPI 3.0.3 specification
  (source file is `src/openapi/openapi.json`, embedded in the binary).

## Docker

```sh
docker build -t rs-http-apilab:latest .
docker run --rm -p 8080:8080 -e LAB_ADMIN_TOKEN=my-token rs-http-apilab:latest
# or
docker compose up -d
```

The image is multi-stage: the builder uses `rust:1.98.1-bookworm`
(pinned toolchain) and the runtime is `debian:bookworm-slim` (the
binary is dynamic, linked against glibc; `ldd` in the Dockerfile and in
the build proves the dependencies). The process runs as a non-root user
(uid 10001) and there is a `HEALTHCHECK` pointing at `/health`.

## Releases and runtime image (binary, no compiling)

Every `v*` tag triggers `.github/workflows/release.yml`: it compiles
with the same pinned toolchain (the Dockerfile `builder` stage), smoke
tests the binary and attaches it to the GitHub release together with
`SHA256SUMS`.

```sh
gh release download v1.0.0 --repo Esl1h/rs-http-apilab   # rs-http-apilab-linux-amd64 + SHA256SUMS
sha256sum -c SHA256SUMS
./rs-http-apilab-linux-amd64
```

The binary is dynamic, linked against glibc (built on Debian bookworm,
glibc 2.36): use it on distributions with glibc >= 2.36 or on the
`debian:bookworm-slim` image.

For a Docker image that compiles nothing, use the runtime image, which
downloads the release binary pinned by tag and checksum (builds in
seconds). The tag and checksum below refer to release `v1.0.0`; update
them together on every new release (the checksum comes from the
release's `SHA256SUMS` file):

```sh
docker build -f Dockerfile.runtime -t rs-http-apilab:latest .
docker run --rm -p 8080:8080 rs-http-apilab:latest
```

`Dockerfile` (compiles from source with the pinned toolchain) and
`Dockerfile.runtime` (uses the release binary with integrity
verification) produce the same server; choose full build
reproducibility or speed.

## Endpoints

Summary; the full contract is in `docs/endpoints.md`.

| Group | Endpoint | Method | Description |
| --- | --- | --- | --- |
| Infra | `/health` | GET | liveness |
| Infra | `/ready` | GET | readiness (forceable to 503) |
| Infra | `/version` | GET | app and rustc versions + build |
| Infra | `/metrics` | GET | Prometheus text |
| Docs | `/openapi.json` | GET | embedded OpenAPI 3.0.3 spec |
| Docs | `/docs` | GET | Swagger UI |
| Auth | `/api/v1/auth/login` | POST | fixed synthetic credentials |
| Auth | `/api/v1/auth/logout` | POST | revokes the token |
| Auth | `/api/v1/auth/me` | GET | synthetic profile (Bearer) |
| Users | `/api/v1/users` | GET/POST | paginated list / creation |
| Users | `/api/v1/users/{id}` | GET/PUT/PATCH/DELETE | CRUD |
| Search | `/api/v1/search?q=&limit=` | GET | search over the synthetic corpus |
| Reports | `/api/v1/reports` | GET | synthetic catalog |
| Reports | `/api/v1/reports/export` | POST | creates a synthetic job |
| Reports | `/api/v1/reports/{id}/download` | GET | synthetic CSV |
| Tickets | `/api/v1/tickets` | POST | creates a ticket |
| Tickets | `/api/v1/tickets/{id}` | GET | fetches a ticket |
| Forms | `/api/v1/forms/feedback` | POST | application/x-www-form-urlencoded |
| Integrations | `/api/v1/webhooks/receive` | POST | summary (size, keys, sha256) |
| Integrations | `/api/v1/graphql` | POST | documented stub (does not run GraphQL) |
| Files | `/api/v1/files/upload` | POST | multipart or octet-stream (never written to disk) |
| Files | `/static/{name}` | GET | embedded allowlist (traversal rejected) |
| Admin | `/admin/status` | GET | non-sensitive counters |
| Admin | `/admin/config` | POST | validated echo, nothing applied |
| Admin | `/internal/debug` | GET | non-sensitive data |
| Lab | `/lab/echo` | GET | reflects method/path/query/allowed headers |
| Lab | `/lab/method` | GET/HEAD/POST/PUT/PATCH/DELETE/OPTIONS | reports the received method |
| Lab | `/lab/slow?ms=` | GET | configurable delay |
| Lab | `/lab/status/{code}` | GET | answers the requested status (200..599) |
| Lab | `/lab/response-marker` | GET | fixed marker `LAB_RESPONSE_MARKER_7f3a` |
| Lab | `/lab/large?bytes=` | GET | N-byte body |
| Lab | `/lab/headers` | GET | custom and security headers |
| Lab | `/lab/stream` | GET | response in parts (native chunked) |
| Lab | `/lab/calls` | GET | buffer of the last requests (token) |
| Lab | `/lab/calls/reset` | POST | clears the buffer (token) |

Every error response follows:

```json
{"error":{"code":"...","message":"...","request_id":"..."}}
```

`X-Request-ID` is returned in every response (using the received value
when valid, or a generated UUID). The `X-Test-Case` header, when sent,
is recorded in the structured log and in the `/lab/calls` buffer to
label test cases.

## Environment variables

| Variable | Default | Description |
| --- | --- | --- |
| `PORT` | `8080` | listening port (binds `0.0.0.0`) |
| `MAX_BODY_BYTES` | `1048576` | per-request body limit (413) |
| `READ_TIMEOUT_MS` | `30000` | header read timeout (native 1 ms granularity) |
| `LAB_ADMIN_TOKEN` | empty | protects `/lab/calls*`; empty disables them (404) |
| `READY_FORCE_FAIL` | empty | any non-empty value forces `/ready` to 503 |
| `LOG_LEVEL` | `info` | `debug`, `info`, `warn` or `error` |
| `MAX_SLOW_MS` | `10000` | ceiling for `/lab/slow?ms=` |
| `MAX_LARGE_BYTES` | `1048576` | ceiling for `/lab/large?bytes=` |
| `LAB_USERNAME` | `admin` | synthetic login username |
| `LAB_PASSWORD` | `lab-password-123456` | synthetic login password (never logged) |

Invalid values in any variable abort the process with an explicit
error.

## Tests

```sh
make test          # runs smoke + errors + limits (shell suite, via curl)
make unit-test     # unit tests of the pure functions (cargo test)
./scripts/test-smoke.sh
./scripts/test-errors.sh
./scripts/test-limits.sh
tests/infra.sh     # any isolated group (all)
LAB_TEST_MODE=errors tests/users.sh
```

Each case prints `PASS/FAIL` with expected vs. actual; any FAIL exits
non-zero. Without `BASE_URL` set, each group starts a fresh server on
`18080` (clean state). With `BASE_URL`, it runs against an external
server.

## Logs

One JSON line per request on stdout: `timestamp`, `request_id`,
`method`, `path`, `query` (sensitive values masked as `[REDACTED]`),
`status`, `duration_ms`, `route`, `remote_addr`, `user_agent`,
`content_length`, `content_type`, `x_forwarded_for`, `x_test_case`.

Never logged or reflected: `Authorization`, `Cookie`, `Set-Cookie`,
passwords or tokens. stdout contains only JSON lines (the startup line
is JSON and connection errors go to stderr; no `grep '^{'` filter
needed, unlike veb).

## Known limitations (operational honesty)

1. **Headers above 16 KiB** are refused by hyper (read buffer fixed at
   16 KiB) with a generic 431, without JSON, without a log entry and
   without a `/lab/calls` record (validated by execution; same layer
   and same behavior as veb/fasthttp in v-http-apilab).
2. **Bodies above `MAX_BODY_BYTES`** always get a 413 with the JSON
   envelope, log entry and `/lab/calls` record (the bounded read
   happens in the application; there is no 64 MiB backend ceiling like
   the one v-http-apilab had: any excess falls into the same 413).
3. **Path traversal with dot segments** (`/static/..%2f..`) is decoded
   and normalized before routing; the request never reaches the
   allowlist. Traversal with backslashes and the like, which reaches
   the handler, gets an explicit 400 (`path_traversal_rejected`).
4. **Streaming is native**: `/lab/stream` uses the axum body stream;
   hyper does the chunked framing. The whole body of the other routes
   is read into memory before the handler (bounded by
   `MAX_BODY_BYTES`).
5. **Multipart is re-parsed from the already-read body**: the axum
   parser (multer) runs over the in-memory bytes; uploads cannot exceed
   `MAX_BODY_BYTES`.
6. **Port already in use**: the process exits with an explicit bind
   error (no interactive prompt, unlike veb).
7. **Determinism**: seed data and responses are deterministic; session
   tokens and job ids (`job-N`) are generated in memory and disappear
   on restart.
8. **Query parsing follows urlencoded rules** (`+` becomes a space), in
   both the query string and the form; the same semantics as veb's
   `ctx.query`/`ctx.form`.

## Repository structure

```
rs-http-apilab/
├── Dockerfile               multi-stage build with the pinned toolchain
├── Dockerfile.runtime       image that uses the release binary (with checksum)
├── docker-compose.yml       isolated backend for local tests
├── Makefile                 shortcuts (build, run, test, docker, fmt, lint, clean)
├── rust-toolchain.toml      pinned Rust toolchain (1.98.1)
├── Cargo.lock               locked dependencies (builds use --locked)
├── build.rs                 bakes rustc --version and build date into the binary
├── .github/workflows/       release.yml: tag v* -> binary + SHA256SUMS on the release
├── scripts/                 build.sh, run.sh, test-smoke.sh, test-errors.sh, test-limits.sh
├── tests/                   lib.sh + groups: infra, auth, users, search-reports,
│                            tickets-forms, webhooks, files, lab
├── docs/                    endpoints.md (contract) and testing.md (how to test)
└── src/                     one module per responsibility:
    ├── main.rs              entry point: config, routes, hyper accept loop
    ├── config.rs            environment variables
    ├── timefmt.rs           RFC3339 timestamps without crates
    ├── seed.rs              deterministic synthetic data (users, reports, ...)
    ├── requestid.rs         X-Request-ID resolution
    ├── logging.rs           structured logging and masking
    ├── state.rs             in-memory state behind a mutex (users, tickets,
    │                        sessions, jobs, metrics, calls buffer)
    ├── router.rs            route table, dispatch, 405+Allow, error envelope
    ├── openapi.rs           embedded OpenAPI spec (include_str!) + Swagger UI
    ├── infra.rs             index, health, ready, version
    ├── auth.rs              synthetic login/logout/me
    ├── users.rs             user CRUD
    ├── search.rs            synthetic search
    ├── reports.rs           reports, export and CSV
    ├── tickets.rs           tickets
    ├── forms.rs             urlencoded form
    ├── webhooks.rs          receive + graphql stub
    ├── admin.rs             status, config (echo), debug
    ├── files.rs             multipart/octet-stream upload + /static allowlist
    ├── lab.rs               testing utilities
    └── metrics.rs           /metrics Prometheus
```

## Backend self-security

Inputs are never executed or interpreted (no SQL, shell or eval): they
are only validated and returned with correct JSON escaping. There is no
disk access beyond reading the binary itself (uploads are analyzed in
memory). No database: restarting the process restores the seed.

## Em português (resumo)

Backend HTTP determinístico, com dados 100% sintéticos, que simula uma
API corporativa interna. É o port em Rust do conceito do
[v-http-apilab](https://github.com/Esl1h/v-http-apilab): mesmos endpoints,
variáveis de ambiente, envelope de erro, buffer de prova `/lab/calls`,
logs estruturados e a mesma suíte de testes em curl, com o framework
trocado de `veb` (V) para `axum` (Rust).

Uso rápido:

```sh
./scripts/build.sh                        # compila bin/rs-http-apilab
LAB_ADMIN_TOKEN=meu-token scripts/run.sh  # sobe em 0.0.0.0:8080
curl http://127.0.0.1:8080/health
make test                                 # suíte smoke + errors + limits
```

Prova de chegada (o motivo de existir do `/lab/calls`): suba com
`LAB_ADMIN_TOKEN`, rotule cada requisição de teste com o header
`X-Test-Case` e consulte o buffer. Presença no buffer prova que a
requisição chegou ao backend; ausência prova que ela foi interrompida
antes (por proxy, WAF, firewall, timeout ou erro de rede). Detalhes em
`docs/testing.md`.

Avisos:

- Ferramenta de laboratório; não é segura como aplicação real e não
  deve ser exposta a redes não controladas.
- Toolchain fixado em `rust-toolchain.toml` (rustc 1.98.1) e
  dependências travadas pelo `Cargo.lock`; builds usam `--locked`.
- A documentação completa do contrato (`docs/endpoints.md`,
  `docs/testing.md` e a especificação OpenAPI) está em inglês; este
  resumo em português é a visão geral.
