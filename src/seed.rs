// seed holds 100% synthetic, deterministic datasets.
//
// Nothing here resembles real people, hosts or credentials: every name,
// email, report and document is generated or hand-written for the lab.
// The data is the same on every start, so a request blocked by an
// intermediary can be diffed against a request that reached the app.
use serde::{Deserialize, Serialize};

pub const APP_NAME: &str = "rs-http-apilab";
pub const APP_VERSION: &str = "1.0.0";

// Toolchain version baked in at build time by build.rs (from $RUSTC).
pub const RUSTC_VERSION: &str = env!("RSAPILAB_RUSTC_VERSION");

// Build date baked in at build time; honors SOURCE_DATE_EPOCH for
// reproducible builds and is formatted by timefmt at runtime.
pub const BUILD_DATE_EPOCH: &str = env!("RSAPILAB_BUILD_DATE_EPOCH");

pub fn build_date() -> String {
    crate::timefmt::rfc3339_from_unix(
        BUILD_DATE_EPOCH
            .parse::<u64>()
            .expect("build.rs always sets a numeric epoch"),
    )
}

// Fixed, harmless marker used by /lab/response-marker to test
// response-body inspection (same value as v-http-lab on purpose).
pub const RESPONSE_MARKER: &str = "LAB_RESPONSE_MARKER_7f3a";

pub const DEPARTMENTS: [&str; 5] = [
    "engineering",
    "finance",
    "operations",
    "support",
    "security",
];

pub const ROLES: [&str; 3] = ["admin", "operator", "viewer"];

pub const SEVERITIES: [&str; 4] = ["low", "medium", "high", "critical"];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct User {
    pub id: u64,
    pub name: String,
    pub email: String,
    pub department: String,
    pub role: String,
}

// users builds the fixed synthetic user table (25 entries).
pub fn users() -> Vec<User> {
    (1..=25)
        .map(|i| {
            let role = if i <= 2 {
                "admin"
            } else if i <= 12 {
                "operator"
            } else {
                "viewer"
            };
            User {
                id: i,
                name: format!("Lab User {i}"),
                email: format!("user{i}@lab.example"),
                department: DEPARTMENTS[(i - 1) as usize % DEPARTMENTS.len()].to_string(),
                role: role.to_string(),
            }
        })
        .collect()
}

#[derive(Clone, Debug, Serialize)]
pub struct Report {
    pub report_id: String,
    pub title: String,
    pub category: String,
    pub rows: u32,
    pub generated_at: String,
}

pub struct ReportDef {
    pub id: &'static str,
    pub title: &'static str,
    pub category: &'static str,
    pub rows: u32,
    pub when: &'static str,
}

// reports is the fixed synthetic report catalog (6 entries).
pub const REPORT_DEFS: [ReportDef; 6] = [
    ReportDef {
        id: "rep-001",
        title: "Monthly Sales Summary (synthetic)",
        category: "sales",
        rows: 8,
        when: "2026-01-31T00:00:00Z",
    },
    ReportDef {
        id: "rep-002",
        title: "Quarterly Finance Overview (synthetic)",
        category: "finance",
        rows: 10,
        when: "2026-01-15T00:00:00Z",
    },
    ReportDef {
        id: "rep-003",
        title: "Infrastructure Utilization (synthetic)",
        category: "operations",
        rows: 12,
        when: "2026-02-05T00:00:00Z",
    },
    ReportDef {
        id: "rep-004",
        title: "Support Tickets by Severity (synthetic)",
        category: "support",
        rows: 6,
        when: "2026-02-10T00:00:00Z",
    },
    ReportDef {
        id: "rep-005",
        title: "Security Audit Checklist (synthetic)",
        category: "security",
        rows: 9,
        when: "2026-03-01T00:00:00Z",
    },
    ReportDef {
        id: "rep-006",
        title: "Employee Rotation Plan (synthetic)",
        category: "hr",
        rows: 5,
        when: "2026-03-15T00:00:00Z",
    },
];

pub fn reports() -> Vec<Report> {
    REPORT_DEFS
        .iter()
        .map(|def| Report {
            report_id: def.id.to_string(),
            title: def.title.to_string(),
            category: def.category.to_string(),
            rows: def.rows,
            generated_at: def.when.to_string(),
        })
        .collect()
}

pub fn report_by_id(id: &str) -> Option<&'static ReportDef> {
    REPORT_DEFS.iter().find(|def| def.id == id)
}

pub struct SearchDoc {
    pub id: &'static str,
    pub title: &'static str,
}

// search_docs is the fixed synthetic document corpus (8 entries).
pub const SEARCH_DOCS: [SearchDoc; 8] = [
    SearchDoc {
        id: "doc-01",
        title: "Runbook: synthetic deployment pipeline",
    },
    SearchDoc {
        id: "doc-02",
        title: "Guide: synthetic incident rotation",
    },
    SearchDoc {
        id: "doc-03",
        title: "Policy: synthetic data retention",
    },
    SearchDoc {
        id: "doc-04",
        title: "Handbook: synthetic onboarding flow",
    },
    SearchDoc {
        id: "doc-05",
        title: "Reference: synthetic API limits",
    },
    SearchDoc {
        id: "doc-06",
        title: "Checklist: synthetic release review",
    },
    SearchDoc {
        id: "doc-07",
        title: "Notebook: synthetic capacity planning",
    },
    SearchDoc {
        id: "doc-08",
        title: "Memo: synthetic lab usage rules",
    },
];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Ticket {
    pub id: u64,
    pub title: String,
    pub description: String,
    pub severity: String,
    pub tags: Vec<String>,
}

// tickets builds the fixed synthetic ticket seed (3 entries).
pub fn tickets() -> Vec<Ticket> {
    vec![
        Ticket {
            id: 1,
            title: "Printer on floor 3 reports synthetic jam".to_string(),
            description: "Synthetic description for lab ticket 1.".to_string(),
            severity: "low".to_string(),
            tags: vec!["hardware".to_string(), "printer".to_string()],
        },
        Ticket {
            id: 2,
            title: "Quarterly report export slower than usual".to_string(),
            description: "Synthetic description for lab ticket 2.".to_string(),
            severity: "medium".to_string(),
            tags: vec!["reports".to_string(), "performance".to_string()],
        },
        Ticket {
            id: 3,
            title: "Suspicious synthetic login pattern flagged".to_string(),
            description: "Synthetic description for lab ticket 3.".to_string(),
            severity: "high".to_string(),
            tags: vec!["security".to_string(), "auth".to_string()],
        },
    ]
}

pub struct StaticFile {
    pub name: &'static str,
    pub content_type: &'static str,
    pub body: &'static str,
}

// static_files is the complete allowlist served by /static/{name}.
// Anything not in this list answers 404; nothing is read from disk.
pub const STATIC_FILES: [StaticFile; 4] = [
    StaticFile {
        name: "robots.txt",
        content_type: "text/plain",
        body: "User-agent: *\nDisallow: /admin/\nDisallow: /internal/\n# synthetic robots file for the lab\n",
    },
    StaticFile {
        name: "style.css",
        content_type: "text/css",
        body: "/* synthetic stylesheet for the lab */\nbody { font-family: monospace; margin: 2rem; }\n.marker { display: none; }\n",
    },
    StaticFile {
        name: "logo.svg",
        content_type: "image/svg+xml",
        body: "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"64\" height=\"64\"><rect width=\"64\" height=\"64\" fill=\"#333\"/><text x=\"8\" y=\"40\" font-size=\"20\" fill=\"#eee\">LAB</text></svg>\n",
    },
    StaticFile {
        name: "data.csv",
        content_type: "text/csv",
        body: "id,label,value\n1,alpha,10\n2,beta,20\n3,gamma,30\n# synthetic dataset for response inspection\n",
    },
];

pub fn static_file_by_name(name: &str) -> Option<&'static StaticFile> {
    STATIC_FILES.iter().find(|f| f.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_is_stable() {
        assert_eq!(users().len(), 25);
        assert_eq!(users()[0].email, "user1@lab.example");
        assert_eq!(users()[24].id, 25);
        assert_eq!(reports().len(), 6);
        assert_eq!(tickets().len(), 3);
        assert_eq!(SEARCH_DOCS.len(), 8);
        assert!(static_file_by_name("robots.txt").is_some());
        assert!(static_file_by_name("nope.txt").is_none());
    }
}
