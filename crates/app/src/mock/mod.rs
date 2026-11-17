//! Mock data that mirrors design/src/build.py. Used with `--mock`.

use chrono::{Duration, Utc};
use model::{
    Accent, BoardData, DevStage, DevState, Issue, Label, Priority, Project, Source, StateType,
    Team, User, Workspace, WorkflowState,
};

pub fn workspaces() -> Vec<Workspace> {
    let ws = |slug: &str, shortcut: u8, accent: Accent| Workspace {
        slug: slug.into(),
        name: slug.into(),
        shortcut,
        accent,
        viewer_id: Some(format!("{slug}-u-ka")),
        position: shortcut as i64,
    };
    vec![ws("acme", 1, Accent::Iris), ws("northwind", 2, Accent::Foam), ws("bluebird", 3, Accent::Gold)]
}

pub fn board(slug: &str) -> BoardData {
    let mut b = Builder::new(slug);
    match slug {
        "acme" => acme(&mut b),
        "northwind" => northwind(&mut b),
        _ => {
            b.team("shop", "SHOP", "Shop");
            b.team("service", "SVC", "Service");
            b.team("it", "IT", "IT");
            b.issue("SHOP-77", "Product page shows the wrong warranty length", "shop", "Todo", Priority::Medium, Some("ka"), &["Bug", "Frontend"], 1, None);
            b.issue("SHOP-75", "Add pay later at checkout", "shop", "Backlog", Priority::Low, None, &["Feature"], 0, None);
            b.issue("SVC-31", "Repair status SMS for finished jobs", "service", "In Progress", Priority::High, Some("ls"), &["Feature"], 3, None);
            b.issue("IT-12", "Renew the wildcard certificate", "it", "Todo", Priority::Urgent, Some("ap"), &["Improvement"], 0, None);
        }
    }
    b.data
}

/// Design v3 02: one team, two projects, everything in Back office is assigned to Kaarel.
fn northwind(b: &mut Builder) {
    use Priority as P;
    b.team("nw", "NW", "Northwind");
    b.project("erp", "Back office");
    b.project("reports", "Reports");
    let erp = Some("erp");
    let ka = Some("ka");
    b.issue("NW-412", "Import supplier price lists from CSV", "nw", "Backlog", P::Low, ka, &["Feature"], 0, erp);
    b.issue("NW-418", "Invoice PDF shows wrong VAT number", "nw", "Todo", P::High, ka, &["Bug"], 2, erp);
    b.issue("NW-415", "Stock count screen for the warehouse tablet", "nw", "Todo", P::Medium, ka, &["Feature", "Design"], 0, erp);
    b.issue("NW-420", "Sync customer balances from accounting every night", "nw", "In Progress", P::Urgent, ka, &["Backend"], 4, erp);
    b.issue("NW-409", "Faster product search on the order form", "nw", "In Progress", P::Medium, ka, &["Improvement"], 1, erp);
    b.issue("NW-401", "Credit note export to accounting", "nw", "Done", P::Medium, ka, &["Feature"], 3, erp);
    b.issue("NW-417", "Monthly sales report as PDF", "nw", "Todo", P::Medium, Some("mt"), &["Feature"], 0, Some("reports"));
    b.dev("NW-418", DevStage::Local, Some("invoice-vat-number"));
    b.dev("NW-420", DevStage::Mr, Some("!57"));
    b.dev("NW-401", DevStage::Prod, None);
}

fn acme(b: &mut Builder) {
    use Priority as P;
    b.team("web", "WEB", "Webshop");
    b.team("plt", "PLT", "Platform");
    b.team("des", "DES", "Design");
    b.project("web", "Webshop launch");
    b.project("infra", "Infra");

    let web = Some("web");
    let infra = Some("infra");
    b.issue("WEB-231", "Season switch banner for winter tyres", "web", "Backlog", P::Low, None, &["Design"], 0, web);
    b.issue("WEB-228", "Merchant Center feed misses GTIN for studded tyres", "web", "Backlog", P::Medium, Some("mt"), &["Bug"], 1, web);
    b.issue("WEB-226", "Parcel price rounds to 2.99 instead of 3.00", "web", "Todo", P::High, Some("ka"), &["Bug", "Backend"], 2, web);
    b.issue("WEB-219", "Add tyre hotel storage step to the booking flow", "web", "Todo", P::Medium, Some("ls"), &["Feature"], 0, web);
    b.issue("WEB-217", "Search Console: duplicate canonical on /tyres pages", "web", "Todo", P::Low, Some("ap"), &["Improvement"], 0, web);
    b.issue("WEB-236", "Show tyre load index next to the size", "web", "Todo", P::Low, Some("mt"), &["Frontend"], 0, web);
    b.issue("WEB-237", "Email reminder one day before the fitting slot", "web", "Todo", P::Medium, Some("ls"), &["Feature"], 1, web);
    b.issue("WEB-238", "Admin: filter bookings by fitting station", "web", "Todo", P::Low, Some("ap"), &["Improvement"], 0, None);
    b.issue("WEB-214", "Refund webhook marks the order paid twice", "web", "In Progress", P::Urgent, Some("ka"), &["Bug", "Backend"], 3, web);
    b.issue("WEB-210", "Tyre size filter drops 17 inch rims", "web", "In Progress", P::High, Some("jk"), &["Bug", "Frontend"], 1, web);
    b.issue("WEB-208", "Booking SMS goes out in English for the et locale", "web", "In Review", P::Medium, Some("ka"), &["Bug"], 2, web);
    b.issue("WEB-205", "Export bookings to CSV from the admin", "web", "Done", P::Medium, Some("mt"), &["Feature"], 0, web);
    b.issue("WEB-201", "Show stock per warehouse on the product page", "web", "Done", P::Low, Some("jk"), &["Feature", "Frontend"], 4, web);

    b.issue("PLT-88", "Move image uploads to Hetzner object storage", "plt", "Backlog", P::None, None, &["Backend"], 0, infra);
    b.issue("PLT-91", "Nightly Postgres backup check fails silently", "plt", "Todo", P::High, Some("ap"), &["Bug"], 1, infra);
    b.issue("PLT-93", "Upgrade TanStack Start to 1.130", "plt", "In Progress", P::Medium, Some("ka"), &["Improvement"], 2, infra);
    b.issue("PLT-90", "Rate limit the login endpoint", "plt", "In Review", P::High, Some("jk"), &["Backend"], 0, infra);
    b.issue("PLT-86", "Sentry: drop noisy hydration warnings", "plt", "In Review", P::Low, Some("ls"), &["Improvement"], 0, infra);
    b.issue("PLT-84", "Caddy config for staging subdomains", "plt", "Done", P::None, Some("mt"), &[], 0, infra);

    b.issue("DES-40", "Tyre hotel icons", "des", "Backlog", P::Low, None, &["Design"], 0, None);
    b.issue("DES-39", "Empty cart illustration", "des", "Backlog", P::Low, Some("ls"), &["Design"], 0, None);
    b.issue("DES-37", "Dark mode for the admin", "des", "Backlog", P::Low, None, &["Design"], 0, None);
    b.issue("DES-36", "Booking calendar states", "des", "Todo", P::Medium, Some("ls"), &["Design"], 1, None);
    b.issue("DES-35", "Winter campaign banners", "des", "Todo", P::Low, Some("ls"), &["Design"], 0, None);
    b.issue("DES-34", "Product card spacing", "des", "In Progress", P::Medium, Some("ls"), &["Design", "Frontend"], 0, None);
    b.issue("DES-30", "Logo for the tyre hotel", "des", "Done", P::Low, Some("ls"), &["Design"], 2, None);

    if let Some(i) = b.data.issues.iter_mut().find(|i| i.identifier == "WEB-214") {
        i.description = "When a customer gets a partial refund, the payment provider sends two webhooks. \
            We handle both, and the second one flips the order back to paid.\n\n\
            ![admin-order-4471.png](https://uploads.linear.app/mock/admin-order-4471.png)"
            .into();
    }

    b.dev("WEB-226", DevStage::Local, Some("parcel-rounding"));
    b.dev("WEB-214", DevStage::Local, Some("refund-webhook"));
    b.dev("WEB-210", DevStage::Mr, Some("!412"));
    b.dev("WEB-208", DevStage::Prod, None);
    b.dev("WEB-205", DevStage::Prod, None);
    b.dev("WEB-201", DevStage::Merged, None);
    b.dev("PLT-93", DevStage::Local, Some("upgrade-tanstack"));
    b.dev("PLT-90", DevStage::Mr, Some("!88"));
}

struct Builder {
    ws: String,
    data: BoardData,
    /// Next sort order per state id, so issues keep the order they are added in.
    next_order: std::collections::HashMap<String, f64>,
}

const STATES: [(&str, StateType, &str); 6] = [
    ("Backlog", StateType::Backlog, "#bec2c8"),
    ("Todo", StateType::Unstarted, "#e2e2e2"),
    ("In Progress", StateType::Started, "#f2c94c"),
    ("In Review", StateType::Started, "#0f783c"),
    ("Done", StateType::Completed, "#5e6ad2"),
    ("Canceled", StateType::Canceled, "#95a2b3"),
];

const USERS: [(&str, &str); 5] = [
    ("ka", "Kaarel"),
    ("mt", "Mari Tamm"),
    ("jk", "Joonas Kask"),
    ("ls", "Liis Saar"),
    ("ap", "Andres Pärn"),
];

/// Ages of the mock issues in hours, used in turn: today, yesterday, a few days, last month, last year.
const AGES_H: [i64; 10] = [2, 26, 72, 820, 5, 130, 30, 290, 9600, 50];

const LABELS: [(&str, &str); 6] = [
    ("Bug", "#eb6f92"),
    ("Feature", "#c4a7e7"),
    ("Improvement", "#9ccfd8"),
    ("Design", "#ebbcba"),
    ("Backend", "#31748f"),
    ("Frontend", "#f6c177"),
];

impl Builder {
    fn new(ws: &str) -> Self {
        let mut data = BoardData {
            viewer_id: Some(format!("{ws}-u-ka")),
            last_sync: Some(Utc::now() - Duration::seconds(12)),
            ..Default::default()
        };
        for (id, name) in USERS {
            data.users.push(User {
                id: format!("{ws}-u-{id}"),
                workspace: ws.into(),
                name: name.into(),
                display_name: name.split(' ').next().unwrap_or(name).to_lowercase(),
                avatar_url: Some(format!(
                    "https://uploads.linear.app/mock/avatar-{}.png",
                    crate::board::card::initials(name).to_lowercase()
                )),
                active: true,
            });
        }
        for (name, color) in LABELS {
            data.labels.push(Label {
                id: format!("{ws}-l-{}", name.to_lowercase()),
                workspace: ws.into(),
                name: name.into(),
                color: color.into(),
                team_id: None,
            });
        }
        Builder { ws: ws.into(), data, next_order: Default::default() }
    }

    fn team(&mut self, id: &str, key: &str, name: &str) {
        let team_id = format!("{}-t-{id}", self.ws);
        self.data.teams.push(Team { id: team_id.clone(), workspace: self.ws.clone(), key: key.into(), name: name.into() });
        for (i, (sname, kind, color)) in STATES.iter().enumerate() {
            self.data.states.push(WorkflowState {
                id: state_id(&team_id, sname),
                team_id: team_id.clone(),
                name: (*sname).into(),
                kind: *kind,
                position: i as f64,
                color: (*color).into(),
            });
        }
    }

    fn dev(&mut self, ident: &str, stage: DevStage, git_ref: Option<&str>) {
        self.data.dev.push(DevState {
            workspace: self.ws.clone(),
            identifier: ident.into(),
            stage,
            git_ref: git_ref.map(Into::into),
            path: git_ref
                .filter(|r| !r.starts_with('!'))
                .map(|r| format!("/home/me/Code/webshop/.claude/worktrees/{r}")),
            url: None,
            updated_at: Utc::now() - Duration::hours(2),
        });
    }

    fn project(&mut self, id: &str, name: &str) {
        self.data.projects.push(Project { id: format!("{}-p-{id}", self.ws), workspace: self.ws.clone(), name: name.into() });
    }

    #[allow(clippy::too_many_arguments)]
    fn issue(
        &mut self,
        ident: &str,
        title: &str,
        team: &str,
        state: &str,
        priority: Priority,
        user: Option<&str>,
        labels: &[&str],
        comments: i64,
        project: Option<&str>,
    ) {
        let ws = self.ws.clone();
        let team_id = format!("{ws}-t-{team}");
        let sid = state_id(&team_id, state);
        let order = self.next_order.entry(sid.clone()).or_insert(1000.0);
        let sort_order = *order;
        *order += 1000.0;
        let now = Utc::now();
        let created_at = now - Duration::hours(AGES_H[self.data.issues.len() % AGES_H.len()]);
        self.data.issues.push(Issue {
            id: format!("{ws}-i-{}", ident.to_lowercase()),
            source: Source::Linear,
            workspace: ws.clone(),
            identifier: ident.into(),
            title: title.into(),
            description: String::new(),
            team_id,
            state_id: sid,
            project_id: project.map(|p| format!("{ws}-p-{p}")),
            assignee_id: user.map(|u| format!("{ws}-u-{u}")),
            priority,
            label_ids: labels.iter().map(|l| format!("{ws}-l-{}", l.to_lowercase())).collect(),
            sort_order,
            comment_count: comments,
            url: Some(format!("https://linear.app/{ws}/issue/{}", ident.to_lowercase())),
            created_at,
            updated_at: created_at.max(now - Duration::hours(3)),
        });
    }
}

fn state_id(team_id: &str, name: &str) -> String {
    format!("{team_id}-s-{}", name.to_lowercase().replace(' ', "-"))
}
