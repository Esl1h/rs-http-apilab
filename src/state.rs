// state owns all shared mutable in-memory state behind a single mutex.
//
// The server answers requests from many tokio tasks, so every mutable
// structure lives here and every access goes through a locking method.
// Restarting the process restores the seed state: there is no database
// and nothing is persisted.
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::Mutex;
use std::time::Instant;

use serde::Serialize;

use crate::seed;
use crate::timefmt;

pub const CALLS_CAPACITY: usize = 200;

#[derive(Clone, Serialize)]
pub struct CallRecord {
    pub request_id: String,
    pub x_test_case: String,
    pub method: String,
    pub path: String,
    pub route: String,
    pub status: u16,
    pub timestamp: String,
}

#[derive(Clone)]
pub struct Session {
    pub username: String,
    pub expires_unix: i64,
}

#[derive(Clone, Serialize)]
pub struct ExportJob {
    pub job_id: String,
    pub report_id: String,
    pub format: String,
    pub created: String,
}

struct Inner {
    users: Vec<seed::User>,
    next_uid: u64,
    tickets: Vec<seed::Ticket>,
    next_tid: u64,
    sessions: HashMap<String, Session>,
    jobs: Vec<ExportJob>,
    next_jid: u64,
    metrics: BTreeMap<String, BTreeMap<String, u64>>,
    req_total: u64,
    calls: VecDeque<CallRecord>,
}

pub struct AppState {
    inner: Mutex<Inner>,
    started: Instant,
    started_at_epoch: u64,
}

impl AppState {
    // new builds the initial state from the synthetic seed.
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                users: seed::users(),
                next_uid: 26,
                tickets: seed::tickets(),
                next_tid: 4,
                sessions: HashMap::new(),
                jobs: Vec::new(),
                next_jid: 1,
                metrics: BTreeMap::new(),
                req_total: 0,
                calls: VecDeque::new(),
            }),
            started: Instant::now(),
            started_at_epoch: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock before UNIX_EPOCH")
                .as_secs(),
        }
    }

    pub fn uptime_seconds(&self) -> i64 {
        self.started.elapsed().as_secs() as i64
    }

    // started_at returns the RFC3339 process start timestamp.
    pub fn started_at(&self) -> String {
        timefmt::rfc3339_from_unix(self.started_at_epoch)
    }

    // ---- users ------------------------------------------------------

    pub fn users_all(&self) -> Vec<seed::User> {
        self.inner.lock().unwrap().users.clone()
    }

    pub fn users_count(&self) -> usize {
        self.inner.lock().unwrap().users.len()
    }

    pub fn user_by_id(&self, id: u64) -> Option<seed::User> {
        self.inner
            .lock()
            .unwrap()
            .users
            .iter()
            .find(|u| u.id == id)
            .cloned()
    }

    // user_add appends a validated user and returns it with its new id.
    pub fn user_add(
        &self,
        name: String,
        email: String,
        department: String,
        role: String,
    ) -> seed::User {
        let mut inner = self.inner.lock().unwrap();
        let user = seed::User {
            id: inner.next_uid,
            name,
            email,
            department,
            role,
        };
        inner.next_uid += 1;
        inner.users.push(user.clone());
        user
    }

    pub fn user_replace(
        &self,
        id: u64,
        name: String,
        email: String,
        department: String,
        role: String,
    ) -> Option<seed::User> {
        let mut inner = self.inner.lock().unwrap();
        let slot = inner.users.iter_mut().find(|u| u.id == id)?;
        *slot = seed::User {
            id,
            name,
            email,
            department,
            role,
        };
        Some(slot.clone())
    }

    // user_patch applies only the provided fields of an existing user.
    pub fn user_patch(&self, id: u64, patch: &HashMap<String, String>) -> Option<seed::User> {
        let mut inner = self.inner.lock().unwrap();
        let slot = inner.users.iter_mut().find(|u| u.id == id)?;
        if let Some(v) = patch.get("name") {
            slot.name = v.clone();
        }
        if let Some(v) = patch.get("email") {
            slot.email = v.clone();
        }
        if let Some(v) = patch.get("department") {
            slot.department = v.clone();
        }
        if let Some(v) = patch.get("role") {
            slot.role = v.clone();
        }
        Some(slot.clone())
    }

    // user_delete removes a user; returns false when the id is unknown.
    pub fn user_delete(&self, id: u64) -> bool {
        let mut inner = self.inner.lock().unwrap();
        match inner.users.iter().position(|u| u.id == id) {
            Some(i) => {
                inner.users.remove(i);
                true
            }
            None => false,
        }
    }

    // ---- tickets ----------------------------------------------------

    pub fn tickets_all(&self) -> Vec<seed::Ticket> {
        self.inner.lock().unwrap().tickets.clone()
    }

    pub fn ticket_by_id(&self, id: u64) -> Option<seed::Ticket> {
        self.inner
            .lock()
            .unwrap()
            .tickets
            .iter()
            .find(|t| t.id == id)
            .cloned()
    }

    pub fn ticket_add(
        &self,
        title: String,
        description: String,
        severity: String,
        tags: Vec<String>,
    ) -> seed::Ticket {
        let mut inner = self.inner.lock().unwrap();
        let ticket = seed::Ticket {
            id: inner.next_tid,
            title,
            description,
            severity,
            tags,
        };
        inner.next_tid += 1;
        inner.tickets.push(ticket.clone());
        ticket
    }

    // ---- sessions ---------------------------------------------------

    pub fn sessions_count(&self) -> usize {
        self.inner.lock().unwrap().sessions.len()
    }

    // session_create issues a synthetic opaque token for `username`.
    pub fn session_create(&self, username: &str, ttl_seconds: i64) -> String {
        let token = format!("apilab-{}", uuid::Uuid::new_v4());
        let expires = now_unix() + ttl_seconds;
        self.inner.lock().unwrap().sessions.insert(
            token.clone(),
            Session {
                username: username.to_string(),
                expires_unix: expires,
            },
        );
        token
    }

    // session_lookup returns the username bound to a live token,
    // dropping the token when it has expired.
    pub fn session_lookup(&self, token: &str) -> Option<String> {
        let mut inner = self.inner.lock().unwrap();
        let session = inner.sessions.get(token)?;
        if now_unix() > session.expires_unix {
            inner.sessions.remove(token);
            return None;
        }
        Some(session.username.clone())
    }

    // session_delete revokes a token; returns false when it did not exist.
    pub fn session_delete(&self, token: &str) -> bool {
        self.inner.lock().unwrap().sessions.remove(token).is_some()
    }

    // ---- export jobs ------------------------------------------------

    pub fn jobs_count(&self) -> usize {
        self.inner.lock().unwrap().jobs.len()
    }

    // job_add records a deterministic export job ("job-N") and returns it.
    pub fn job_add(&self, report_id: &str, format: &str) -> ExportJob {
        let mut inner = self.inner.lock().unwrap();
        let job = ExportJob {
            job_id: format!("job-{}", inner.next_jid),
            report_id: report_id.to_string(),
            format: format.to_string(),
            created: timefmt::now_rfc3339(),
        };
        inner.next_jid += 1;
        inner.jobs.push(job.clone());
        job
    }

    // ---- metrics ----------------------------------------------------

    // record_metric counts one request for a route pattern and status
    // code; `route` is e.g. "/api/v1/users/:id" or "unmatched".
    pub fn record_metric(&self, route: &str, status: u16) {
        let mut inner = self.inner.lock().unwrap();
        inner.req_total += 1;
        let counter = inner
            .metrics
            .entry(route.to_string())
            .or_default()
            .entry(status.to_string())
            .or_insert(0);
        *counter += 1;
    }

    // metrics_counters_text renders the rs_http_apilab_requests_total
    // lines in Prometheus text format (routes and codes sorted).
    pub fn metrics_counters_text(&self) -> String {
        let inner = self.inner.lock().unwrap();
        let mut out = String::new();
        for (route, statuses) in &inner.metrics {
            for (code, n) in statuses {
                out.push_str(&format!(
                    "rs_http_apilab_requests_total{{route=\"{route}\",status=\"{code}\"}} {n}\n"
                ));
            }
        }
        out
    }

    // requests_total returns how many requests reached the dispatch
    // layer (everything that produced a log line).
    pub fn requests_total(&self) -> u64 {
        self.inner.lock().unwrap().req_total
    }

    // ---- /lab/calls ring ---------------------------------------------

    // record_call appends one request to the bounded ring (oldest
    // entries are dropped once the capacity is reached).
    pub fn record_call(
        &self,
        request_id: &str,
        test_case: &str,
        method: &str,
        path: &str,
        route: &str,
        status: u16,
    ) {
        let rec = CallRecord {
            request_id: request_id.to_string(),
            x_test_case: test_case.to_string(),
            method: method.to_string(),
            path: path.to_string(),
            route: route.to_string(),
            status,
            timestamp: timefmt::now_rfc3339(),
        };
        let mut inner = self.inner.lock().unwrap();
        inner.calls.push_back(rec);
        while inner.calls.len() > CALLS_CAPACITY {
            inner.calls.pop_front();
        }
    }

    // calls_all returns a chronological copy of the ring (oldest first).
    pub fn calls_all(&self) -> Vec<CallRecord> {
        self.inner.lock().unwrap().calls.iter().cloned().collect()
    }

    // calls_reset clears the ring; returns how many entries were dropped.
    pub fn calls_reset(&self) -> usize {
        let mut inner = self.inner.lock().unwrap();
        let n = inner.calls.len();
        inner.calls.clear();
        n
    }
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before UNIX_EPOCH")
        .as_secs() as i64
}
