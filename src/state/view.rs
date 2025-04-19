use std::fmt::{self, Display};

use crate::fly_rust::resource_organizations::OrganizationFilter;
use crate::logs::LogOptions;

#[derive(Clone, Debug)]
pub enum View {
    Organizations { filter: OrganizationFilter },
    // org_id is used for highlighting the correct row navigating back,
    // org_slug is used for filtering the apps and as part of breadcrumb
    Apps { org_id: String, org_slug: String },
    // app_id is used for highlighting the correct row navigating back,
    // app_name is used for api calls and as part of breadcrumb
    Machines { app_id: String, app_name: String },
    Volumes { app_id: String, app_name: String },
    Secrets { app_id: String, app_name: String },
    // LogOptions already have app_name
    AppLogs { app_id: String, opts: LogOptions },
    // LogOptions already have vm_id
    MachineLogs { opts: LogOptions },
}

impl View {
    pub fn headers(&self) -> &[&str] {
        match self {
            View::Organizations { .. } => &["Name", "Viewer Role", "Slug", "Type"],
            View::Apps { .. } => &["Name", "Organization", "Status", "Latest Deployment"],
            View::Machines { .. } => &["Id", "Name", "State", "Region", "Updated At"],
            View::Volumes { .. } => &[
                "Id",
                "State",
                "Name",
                "Size",
                "Region",
                "Zone",
                "Encrypted",
                "Attached VM",
                "Created At",
            ],
            View::Secrets { .. } => &["Name", "Digest", "Created At"],
            _ => &[],
        }
    }

    pub fn to_breadcrumb(&self) -> String {
        match self {
            View::Organizations { .. } => String::from("organization"),
            View::Apps { .. } => String::from("app"),
            View::Machines { .. } => String::from("machines"),
            View::Volumes { .. } => String::from("volumes"),
            View::Secrets { .. } => String::from("secrets"),
            _ => String::from("logs"),
        }
    }

    pub fn to_scope(&self) -> String {
        match self {
            View::Organizations { filter } => String::from(if filter.is_admin_only() {
                "admin-only"
            } else {
                "all"
            }),
            View::Apps { org_slug, .. } => String::from(org_slug),
            View::Machines { app_name, .. } => String::from(app_name),
            View::Volumes { app_name, .. } => String::from(app_name),
            View::Secrets { app_name, .. } => String::from(app_name),
            View::AppLogs { opts, .. } => opts.clone().app_name,
            View::MachineLogs { opts, .. } => opts.clone().vm_id.unwrap(),
        }
    }
}

impl Display for View {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            View::Organizations { .. } => write!(f, "Organizations"),
            View::Apps { .. } => write!(f, "Apps"),
            View::Machines { .. } => write!(f, "Machines"),
            View::Volumes { .. } => write!(f, "Volumes"),
            View::Secrets { .. } => write!(f, "Secrets"),
            _ => write!(f, "logs"),
        }
    }
}
