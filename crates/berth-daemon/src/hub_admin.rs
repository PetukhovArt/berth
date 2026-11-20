use anyhow::Result;
use berth_core::ServerEvent;
use berth_hub::{AdminChange, AdminView, Me, Permission};
use serde::Serialize;

use crate::channels::hub;
use crate::config::Config;

#[derive(Debug, Serialize)]
#[serde(tag = "state", rename_all = "lowercase")]
pub(crate) enum HubView {
    Off,
    Legacy,
    Joined {
        me: Me,
        admin: Option<Box<AdminView>>,
    },
}

pub(crate) async fn view() -> Result<HubView> {
    let config = Config::load();
    if config.hub_url.is_none() || config.hub_token.is_none() {
        return Ok(HubView::Off);
    }
    let hub = hub()?;
    let me = hub.me().await?;
    if me.role.is_none() {
        return Ok(HubView::Legacy);
    }
    let admin = if me.holds(Permission::AdminManage) {
        Some(Box::new(hub.admin_view().await?))
    } else {
        None
    };
    Ok(HubView::Joined { me, admin })
}

pub(crate) async fn change(json: &str) -> Result<HubView> {
    let change: AdminChange = serde_json::from_str(json)?;
    hub()?.admin_change(&change).await?;
    view().await
}

pub(crate) fn reply(req_id: u64, result: Result<HubView>) -> ServerEvent {
    match result.and_then(|view| Ok(serde_json::to_string(&view)?)) {
        Ok(json) => ServerEvent::Hub { req_id, json },
        Err(e) => ServerEvent::Error {
            req_id: Some(req_id),
            message: format!("{e:#}"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use berth_hub::RoleRef;
    use serde_json::json;

    #[test]
    fn the_view_is_tagged_by_state_as_the_app_reads_it() {
        assert_eq!(
            serde_json::to_value(HubView::Off).unwrap(),
            json!({ "state": "off" })
        );
        assert_eq!(
            serde_json::to_value(HubView::Legacy).unwrap(),
            json!({ "state": "legacy" })
        );
        let me = Me {
            id: "m-1".into(),
            name: "ann".into(),
            role: Some(RoleRef {
                id: "guest".into(),
                name: "Guest".into(),
            }),
            permissions: vec!["channels.read".into()],
        };
        assert_eq!(
            serde_json::to_value(HubView::Joined { me, admin: None }).unwrap(),
            json!({
                "state": "joined",
                "me": { "id": "m-1", "name": "ann", "role": { "id": "guest", "name": "Guest" }, "permissions": ["channels.read"] },
                "admin": null,
            })
        );
    }
}
