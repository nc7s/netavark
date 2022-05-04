use crate::error::{NetavarkError, NetavarkResult};
use crate::network::internal_types::{
    PortForwardConfig, SetupNetwork, TearDownNetwork, TeardownPortForward,
};
use futures::executor::block_on;
use log::{debug, info};
use std::env;
use zbus::Connection;

pub mod firewalld;
pub mod iptables;
mod varktables;

// Firewall drivers have the ability to set up per-network firewall forwarding
// and port mappings.
pub trait FirewallDriver {
    // Set up firewall rules for the given network,
    fn setup_network(&self, network_setup: SetupNetwork) -> NetavarkResult<()>;
    // Tear down firewall rules for the given network.
    fn teardown_network(&self, tear: TearDownNetwork) -> NetavarkResult<()>;

    // Set up port-forwarding firewall rules for a given container.
    fn setup_port_forward(&self, setup_pw: PortForwardConfig) -> NetavarkResult<()>;
    // Tear down port-forwarding firewall rules for a single container.
    fn teardown_port_forward(&self, teardown_pf: TeardownPortForward) -> NetavarkResult<()>;
}

// Types of firewall backend
enum FirewallImpl {
    Iptables,
    Firewalld(Connection),
    Nftables,
}

// What firewall implementations does this system support?
fn get_firewall_impl() -> NetavarkResult<FirewallImpl> {
    // First, check the NETAVARK_FW env var.
    // It respects "firewalld", "iptables", "nftables".
    if let Ok(var) = env::var("NETAVARK_FW") {
        debug!("Forcibly using firewall driver {}", var);
        match var.to_lowercase().as_str() {
            "firewalld" => {
                let conn = match block_on(Connection::system()) {
                    Ok(c) => c,
                    Err(e) => {
                        return Err(NetavarkError::wrap_str(
                            "Error retrieving dbus connection for requested firewall backend",
                            e.into(),
                        ))
                    }
                };
                return Ok(FirewallImpl::Firewalld(conn));
            }
            "iptables" => return Ok(FirewallImpl::Iptables),
            "nftables" => return Ok(FirewallImpl::Nftables),
            any => {
                return Err(NetavarkError::Message(format!(
                    "Must provide a valid firewall backend, got {}",
                    any
                )))
            }
        }
    }

    // Until firewalld 1.1.0 with support for self-port forwarding lands:
    // Just use iptables
    Ok(FirewallImpl::Iptables)

    // Is firewalld running?
    // let conn = match block_on(Connection::system()) {
    //     Ok(conn) => conn,
    //     Err(_) => return FirewallImpl::Iptables,
    // };
    // match block_on(conn.call_method(
    //     Some("org.freedesktop.DBus"),
    //     "/org/freedesktop/DBus",
    //     Some("org.freedesktop.DBus"),
    //     "GetNameOwner",
    //     &"org.fedoraproject.FirewallD1",
    // )) {
    //     Ok(_) => FirewallImpl::Firewalld(conn),
    //     Err(_) => FirewallImpl::Iptables,
    // }
}

// Get the preferred firewall implementation for the current system
// configuration.
pub fn get_supported_firewall_driver() -> NetavarkResult<Box<dyn FirewallDriver>> {
    match get_firewall_impl() {
        Ok(fw) => match fw {
            FirewallImpl::Iptables => {
                info!("Using iptables firewall driver");
                iptables::new()
            }
            FirewallImpl::Firewalld(conn) => {
                info!("Using firewalld firewall driver");
                firewalld::new(conn)
            }
            FirewallImpl::Nftables => {
                info!("Using nftables firewall driver");
                Err(NetavarkError::msg_str(
                    "nftables support presently not available",
                ))
            }
        },
        Err(e) => Err(e),
    }
}
