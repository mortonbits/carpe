//! networks to connect to

use crate::{
  carpe_error::CarpeError,
  configs_network::{
    override_upstream_node, set_network_configs, set_waypoint, set_waypoint_from_upstream,
    NetworkProfile, Networks,
  },
};
use diem_types::waypoint::Waypoint;
use ol_types::rpc_playlist;
use url::Url;

#[tauri::command]
pub fn toggle_network(network: Networks) -> Result<NetworkProfile, CarpeError> {
  set_network_configs(network)
}

#[tauri::command(async)]
pub fn get_networks() -> Result<NetworkProfile, CarpeError> {
  NetworkProfile::new()
}

#[tauri::command]
pub fn override_playlist(url: Url) -> Result<NetworkProfile, CarpeError> {
  let fpl = rpc_playlist::FullnodePlaylist::http_fetch_playlist(url)?;
  fpl.update_config_file(None)?;
  NetworkProfile::new()
}

#[tauri::command]
pub fn force_upstream(url: Url) -> Result<NetworkProfile, CarpeError> {
  override_upstream_node(url).map_err(|e| CarpeError::misc(&e.to_string()))?;
  NetworkProfile::new()
}

#[tauri::command]
pub fn force_waypoint(wp: Waypoint) -> Result<NetworkProfile, CarpeError> {
  set_waypoint(wp).map_err(|e| CarpeError::misc(&e.to_string()))?;
  NetworkProfile::new()
}

#[tauri::command(async)]
pub async fn refresh_waypoint() -> Result<NetworkProfile, CarpeError> {
  dbg!("refresh_waypoint");
  set_waypoint_from_upstream().await?;
  NetworkProfile::new()
}

// #[tauri::command]
// pub fn refresh_waypoint() -> Result<NetworkProfile, CarpeError> {
//   dbg!("refresh_waypoint");
//   set_waypoint_from_upstream()?;
//   NetworkProfile::new()
// }
