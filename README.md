# rs-http-apilab

Backend HTTP determinístico, com dados 100% sintéticos, que simula uma API
corporativa interna. Rodando o binário, a porta abre e a API começa a
responder imediatamente: sem banco, sem migrações, sem arquivos de
configuração (reiniciar restaura o estado inicial).

É o port em Rust do conceito do [v-http-lab](https://github.com/Esl1h/v-http-lab):
mesmos endpoints, mesmas variáveis de ambiente, mesmo envelope de erro,
mesma prova de chegada (`/lab/calls`) e a mesma suíte de testes em shell,
com o framework HTTP trocado de `veb` (V) para `axum` (Rust).

Documentação interativa (Swagger UI) em `/docs` e especificação OpenAPI
3.0.3 embutida no binário em `/openapi.json`.

> **Aviso**: ferramenta de laboratório. Não é um template de produção, não é
> segura como aplicação real e não deve ser exposta a redes não controladas.
> A autenticação é simulada, os dados são sintéticos e nada é persistido.

## Versão do compilador e framework HTTP

| Item | Valor fixado |
| --- | --- |
| Linguagem | Rust, `rustc 1.98.1` (fixado por `rust-toolchain.toml`) |
| Runtime assíncrono | tokio 1.53.2 |
| Framework HTTP | axum 0.8.9 (sobre hyper 1.12.0 + hyper-util 0.1.21) |
| Outras dependências | serde 1.0.229, serde_json 1.0.154, uuid 1.28.0, sha2 0.10.9, form_urlencoded 1.2.2, percent-encoding 2.3.2, tokio-stream 0.1.19, tower 0.5.3 |
| Fixação | versões exatas travadas pelo `Cargo.lock` commitado; builds usam `--locked` |

Justificativas:

1. `axum` é o framework web mantido sobre hyper 1.x/tokio com roteamento,
   multipart nativo e corpo de resposta em stream (o `/lab/stream` do
   v-http-lab exigia takeover manual da conexão; aqui o streaming é nativo).
2. O servidor não usa `axum::serve` (que não expõe configuração HTTP): o
   accept loop é próprio, com `header_read_timeout` (mapeia
   `READ_TIMEOUT_MS` com granularidade nativa de milissegundos) e buffer de
   leitura de 16 KiB (mesmo limite de headers documentado no v-http-lab).
3. O roteamento é uma tabela própria dentro de um catch-all (a mesma
   arquitetura do v-http-lab): assim toda requisição, inclusive 404/405 e
   métodos exóticos como `TRACE`, recebe o envelope JSON, o header `Allow`
   quando cabível, linha de log e registro em `/lab/calls`.

## Build e execução (local)

Requisitos: Rust 1.98.1 (o `rust-toolchain.toml` resolve via rustup), `curl`,
`bash`.

```sh
./scripts/build.sh          # compila bin/rs-http-apilab (grava a versão do rustc no binário)
./scripts/run.sh            # sobe o servidor (PORT=8080 por padrão)
```

Exemplos:

```sh
PORT=9090 scripts/run.sh
LAB_ADMIN_TOKEN=meu-token scripts/run.sh   # habilita /lab/calls
READY_FORCE_FAIL=1 scripts/run.sh          # /ready passa a responder 503
```

Documentação ao vivo:

- `http://localhost:8080/docs`: Swagger UI (recursos da interface via CDN;
  a API em si é autocontida).
- `http://localhost:8080/openapi.json`: especificação OpenAPI 3.0.3
  (o arquivo fonte é `src/openapi/openapi.json`, embutido no binário).

## Docker

```sh
docker build -t rs-http-apilab:latest .
docker run --rm -p 8080:8080 -e LAB_ADMIN_TOKEN=meu-token rs-http-apilab:latest
# ou
docker compose up -d
```

A imagem é multi-stage: o builder usa `rust:1.98.1-bookworm` (toolchain
fixado) e o runtime é `debian:bookworm-slim` (o binário é dinâmico, linkado
à glibc; `ldd` no Dockerfile e no build comprovam as dependências). O
processo roda como usuário não-root (uid 10001) e há um `HEALTHCHECK`
apontando para `/health`.

## Releases e imagem de runtime (binário, sem compilar)

Cada tag `v*` dispara o workflow `.github/workflows/release.yml`: ele
compila com o mesmo toolchain fixado (estágio `builder` do Dockerfile),
testa o binário e o anexa ao release do GitHub junto com `SHA256SUMS`.

```sh
gh release download v1.0.0 --repo Esl1h/rs-http-apilab   # rs-http-apilab-linux-amd64 + SHA256SUMS
sha256sum -c SHA256SUMS
./rs-http-apilab-linux-amd64
```

O binário é dinâmico, linkado à glibc (compilado em Debian bookworm,
glibc 2.36): use em distribuições com glibc >= 2.36 ou na imagem
`debian:bookworm-slim`.

Para um Docker que não compila nada, use a imagem de runtime, que baixa o
binário do release fixado por tag e checksum (build em segundos). Antes do
primeiro release, atualize `RELEASE_TAG`/`RELEASE_SHA256` do
`Dockerfile.runtime` para valores reais:

```sh
docker build -f Dockerfile.runtime -t rs-http-apilab:latest .
docker run --rm -p 8080:8080 rs-http-apilab:latest
```

`Dockerfile` (compila do código com o toolchain fixado) e
`Dockerfile.runtime` (usa o binário do release com verificação de
integrity) produzem o mesmo servidor; escolha conforme preferir
reprodutibilidade total do build ou velocidade.

## Endpoints

Resumo; contrato completo em `docs/endpoints.md`.

| Grupo | Endpoint | Método | Descrição |
| --- | --- | --- | --- |
| Infra | `/health` | GET | liveness |
| Infra | `/ready` | GET | readiness (forçável para 503) |
| Infra | `/version` | GET | versões do app e do rustc + build |
| Infra | `/metrics` | GET | texto Prometheus |
| Docs | `/openapi.json` | GET | especificação OpenAPI 3.0.3 embutida |
| Docs | `/docs` | GET | Swagger UI |
| Auth | `/api/v1/auth/login` | POST | credenciais sintéticas fixas |
| Auth | `/api/v1/auth/logout` | POST | revoga o token |
| Auth | `/api/v1/auth/me` | GET | perfil sintético (Bearer) |
| Users | `/api/v1/users` | GET/POST | lista paginada / criação |
| Users | `/api/v1/users/{id}` | GET/PUT/PATCH/DELETE | CRUD |
| Busca | `/api/v1/search?q=&limit=` | GET | busca no corpus sintético |
| Relatórios | `/api/v1/reports` | GET | catálogo sintético |
| Relatórios | `/api/v1/reports/export` | POST | cria job sintético |
| Relatórios | `/api/v1/reports/{id}/download` | GET | CSV sintético |
| Tickets | `/api/v1/tickets` | POST | cria ticket |
| Tickets | `/api/v1/tickets/{id}` | GET | consulta ticket |
| Forms | `/api/v1/forms/feedback` | POST | application/x-www-form-urlencoded |
| Integrações | `/api/v1/webhooks/receive` | POST | resumo (tamanho, chaves, sha256) |
| Integrações | `/api/v1/graphql` | POST | stub documentado (não executa GraphQL) |
| Arquivos | `/api/v1/files/upload` | POST | multipart ou octet-stream (não grava em disco) |
| Arquivos | `/static/{name}` | GET | allowlist embutida (traversal rejeitado) |
| Admin | `/admin/status` | GET | contadores não sensíveis |
| Admin | `/admin/config` | POST | eco validado, nada é aplicado |
| Admin | `/internal/debug` | GET | dados não sensíveis |
| Lab | `/lab/echo` | GET | reflete método/path/query/headers permitidos |
| Lab | `/lab/method` | GET/HEAD/POST/PUT/PATCH/DELETE/OPTIONS | informa o método recebido |
| Lab | `/lab/slow?ms=` | GET | atraso configurável |
| Lab | `/lab/status/{code}` | GET | responde o status pedido (200..599) |
| Lab | `/lab/response-marker` | GET | marcador fixo `LAB_RESPONSE_MARKER_7f3a` |
| Lab | `/lab/large?bytes=` | GET | corpo de N bytes |
| Lab | `/lab/headers` | GET | headers customizados e de segurança |
| Lab | `/lab/stream` | GET | resposta em partes (chunked nativo) |
| Lab | `/lab/calls` | GET | buffer das últimas requisições (token) |
| Lab | `/lab/calls/reset` | POST | limpa o buffer (token) |

Toda resposta de erro segue:

```json
{"error":{"code":"...","message":"...","request_id":"..."}}
```

`X-Request-ID` é devolvido em toda resposta (usando o valor recebido quando
válido, ou um UUID gerado). O header `X-Test-Case`, quando enviado, é gravado
no log estruturado e no buffer `/lab/calls` para rotular casos de teste.

## Variáveis de ambiente

| Variável | Padrão | Descrição |
| --- | --- | --- |
| `PORT` | `8080` | porta de escuta (bind em `0.0.0.0`) |
| `MAX_BODY_BYTES` | `1048576` | limite de body por requisição (413) |
| `READ_TIMEOUT_MS` | `30000` | timeout de leitura de headers (granularidade nativa de 1 ms) |
| `LAB_ADMIN_TOKEN` | vazio | protege `/lab/calls*`; vazio desabilita (404) |
| `READY_FORCE_FAIL` | vazio | qualquer valor não vazio força `/ready` a 503 |
| `LOG_LEVEL` | `info` | `debug`, `info`, `warn` ou `error` |
| `MAX_SLOW_MS` | `10000` | teto do `/lab/slow?ms=` |
| `MAX_LARGE_BYTES` | `1048576` | teto do `/lab/large?bytes=` |
| `LAB_USERNAME` | `admin` | usuário sintético do login |
| `LAB_PASSWORD` | `lab-password-123456` | senha sintética do login (nunca logada) |

Valores inválidos em qualquer variável abortam o processo com erro explícito.

## Testes

```sh
make test          # executa smoke + errors + limits (suíte shell, via curl)
make unit-test     # testes unitários das funções puras (cargo test)
./scripts/test-smoke.sh
./scripts/test-errors.sh
./scripts/test-limits.sh
tests/infra.sh     # qualquer grupo isolado (all)
LAB_TEST_MODE=errors tests/users.sh
```

Cada caso imprime `PASS/FAIL` com esperado vs. obtido; qualquer FAIL encerra
com código de saída não zero. Sem `BASE_URL` definida, cada grupo sobe um
servidor novo em `18080` (estado limpo). Com `BASE_URL`, roda contra um
servidor externo.

## Logs

Uma linha JSON por requisição em stdout: `timestamp`, `request_id`, `method`,
`path`, `query` (valores sensíveis mascarados como `[REDACTED]`), `status`,
`duration_ms`, `route`, `remote_addr`, `user_agent`, `content_length`,
`content_type`, `x_forwarded_for`, `x_test_case`.

Nunca são logados ou refletidos: `Authorization`, `Cookie`, `Set-Cookie`,
senhas ou tokens. O stdout contém apenas linhas JSON (a linha de startup é
JSON e os erros de conexão ficam em stderr; sem filtro `grep '^{'`
necessário, ao contrário do veb).

## Limitações conhecidas (honestidade operacional)

1. **Headers acima de 16 KiB** são recusados pelo hyper (buffer de leitura
   fixado em 16 KiB) com 431 genérico, sem JSON, sem log e sem registro em
   `/lab/calls` (validado por execução; mesma camada e mesmo comportamento
   do veb/fasthttp no v-http-lab).
2. **Corpos acima de `MAX_BODY_BYTES`** recebem sempre 413 com envelope
   JSON, log e registro em `/lab/calls` (a leitura limitada acontece na
   aplicação; não existe o teto de 64 MiB do backend que o v-http-lab
   tinha: qualquer excesso cai no mesmo 413).
3. **Path traversal com dot-segments** (`/static/..%2f..`) é decodificado
   e normalizado antes do roteamento; a requisição nunca alcança a
   allowlist. Traversal com barras invertidas e afins, que chegam ao
   handler, recebem 400 explícito (`path_traversal_rejected`).
4. **Streaming é nativo**: `/lab/stream` usa o corpo em stream do axum;
   o enquadramento chunked é do hyper. O corpo inteiro das outras rotas é
   lido em memória antes do handler (bounded por `MAX_BODY_BYTES`).
5. **Multipart é re-parseado do corpo já lido**: o parser do axum (multer)
   roda sobre os bytes em memória; uploads não excedem `MAX_BODY_BYTES`.
6. **Porta em uso**: o processo sai com erro de bind explícito (sem
   prompt interativo, ao contrário do veb).
7. **Determinismo**: dados e respostas do seed são determinísticos; tokens
   de sessão e ids de job (`job-N`) são gerados em memória e somem no
   restart.
8. **QUERY parsing segue urlencoded** (`+` vira espaço), tanto na query
   string quanto no form; é a mesma semântica do `ctx.query`/`ctx.form`
   do veb.

## Estrutura do repositório

```
rs-http-apilab/
├── Dockerfile               build multi-stage com o toolchain fixado
├── Dockerfile.runtime       imagem que usa o binário do release (com checksum)
├── docker-compose.yml       backend isolado para teste local
├── Makefile                 atalhos (build, run, test, docker, fmt, lint, clean)
├── rust-toolchain.toml      toolchain Rust fixado (1.98.1)
├── Cargo.lock               dependências travadas (build com --locked)
├── build.rs                 grava rustc --version e build date no binário
├── .github/workflows/       release.yml: tag v* -> binário + SHA256SUMS no release
├── scripts/                 build.sh, run.sh, test-smoke.sh, test-errors.sh, test-limits.sh
├── tests/                   lib.sh + grupos: infra, auth, users, search-reports,
│                            tickets-forms, webhooks, files, lab
├── docs/                    endpoints.md (contrato) e testing.md (como testar)
└── src/                     um módulo por responsabilidade:
    ├── main.rs              entrada: config, rotas, accept loop hyper
    ├── config.rs            variáveis de ambiente
    ├── timefmt.rs           timestamps RFC3339 sem crates
    ├── seed.rs              dados sintéticos determinísticos (usuários, relatórios...)
    ├── requestid.rs         resolução do X-Request-ID
    ├── logging.rs           log estruturado e mascaramento
    ├── state.rs             estado em memória com mutex (users, tickets, sessões,
    │                        jobs, métricas, buffer de calls)
    ├── router.rs            tabela de rotas, dispatch, 405+Allow, envelope de erro
    ├── openapi.rs           spec OpenAPI embutida (include_str!) + Swagger UI
    ├── infra.rs             index, health, ready, version
    ├── auth.rs              login/logout/me sintéticos
    ├── users.rs             CRUD de usuários
    ├── search.rs            busca sintética
    ├── reports.rs           relatórios, export e CSV
    ├── tickets.rs           tickets
    ├── forms.rs             form urlencoded
    ├── webhooks.rs          receive + stub graphql
    ├── admin.rs             status, config (eco), debug
    ├── files.rs             upload multipart/octet-stream + allowlist /static
    ├── lab.rs               utilitários de teste
    └── metrics.rs           /metrics Prometheus
```

## Segurança do próprio backend

Entradas nunca são executadas nem interpretadas (não há SQL, shell ou
eval): são apenas validadas e devolvidas com escape JSON correto. Não há
acesso a disco além da leitura do próprio binário (uploads são analisados
em memória). Sem banco de dados: reiniciar o processo restaura o seed.
