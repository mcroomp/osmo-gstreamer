use std::{io, net::Ipv4Addr};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Entry {
    address: Ipv4Addr,
    mac: [u8; 6],
}

pub(super) fn find_ipv4_for_mac(mac: [u8; 6]) -> io::Result<Vec<Ipv4Addr>> {
    Ok(matching_addresses(platform::read_entries()?, mac))
}

fn matching_addresses(entries: impl IntoIterator<Item = Entry>, mac: [u8; 6]) -> Vec<Ipv4Addr> {
    let mut addresses: Vec<_> = entries
        .into_iter()
        .filter_map(|entry| (entry.mac == mac).then_some(entry.address))
        .collect();
    addresses.sort_unstable();
    addresses.dedup();
    addresses
}

#[cfg(windows)]
mod platform {
    use super::Entry;
    use std::{io, net::Ipv4Addr, ptr};
    use windows::{
        Win32::{
            Foundation::WIN32_ERROR,
            NetworkManagement::IpHelper::{
                FreeMibTable, GetIpNetTable2, MIB_IPNET_ROW2, MIB_IPNET_TABLE2,
            },
            Networking::WinSock::AF_INET,
        },
        core::Error as WindowsError,
    };

    struct MibTable(*mut MIB_IPNET_TABLE2);

    impl Drop for MibTable {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe { FreeMibTable(self.0.cast()) };
            }
        }
    }

    pub(super) fn read_entries() -> io::Result<Vec<Entry>> {
        let mut table = ptr::null_mut();
        let result = unsafe { GetIpNetTable2(AF_INET, &mut table) };
        let table = MibTable(table);
        if result != WIN32_ERROR(0) {
            return Err(WindowsError::from(result).into());
        }
        if table.0.is_null() {
            return Err(io::Error::other(
                "Windows returned a null IP neighbor table",
            ));
        }

        let count = unsafe { (*table.0).NumEntries as usize };
        let rows = unsafe {
            let first = ptr::addr_of!((*table.0).Table).cast::<MIB_IPNET_ROW2>();
            std::slice::from_raw_parts(first, count)
        };
        let mut entries = Vec::with_capacity(count);
        for row in rows {
            if row.PhysicalAddressLength != 6 {
                continue;
            }
            let address = unsafe { row.Address.Ipv4.sin_addr };
            entries.push(Entry {
                address: Ipv4Addr::from(address),
                mac: row.PhysicalAddress[..6].try_into().expect("length checked"),
            });
        }
        drop(table);
        Ok(entries)
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use super::Entry;
    use futures::TryStreamExt;
    use netlink_packet_route::{
        AddressFamily,
        neighbour::{NeighbourAddress, NeighbourAttribute},
    };
    use rtnetlink::new_connection;
    use std::{io, time::Duration};
    use tokio::runtime::Builder;

    pub(super) fn read_entries() -> io::Result<Vec<Entry>> {
        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(io::Error::other)?;
        runtime
            .block_on(tokio::time::timeout(
                Duration::from_secs(5),
                read_entries_async(),
            ))
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "neighbor-table query timed out"))?
            .map_err(io::Error::other)
    }

    async fn read_entries_async() -> Result<Vec<Entry>, Box<dyn std::error::Error + Send + Sync>> {
        let (connection, handle, _) = new_connection()?;
        tokio::spawn(connection);
        let mut messages = handle.neighbours().get().execute();
        let mut entries = Vec::new();
        while let Some(message) = messages.try_next().await? {
            if message.header.family != AddressFamily::Inet {
                continue;
            }
            let mut address = None;
            let mut mac = None;
            for attribute in message.attributes {
                match attribute {
                    NeighbourAttribute::Destination(NeighbourAddress::Inet(ip)) => {
                        address = Some(ip);
                    }
                    NeighbourAttribute::LinkLayerAddress(bytes) if bytes.len() == 6 => {
                        mac = Some(bytes.try_into().expect("length checked"));
                    }
                    _ => {}
                }
            }
            if let (Some(address), Some(mac)) = (address, mac) {
                entries.push(Entry { address, mac });
            }
        }
        Ok(entries)
    }
}

#[cfg(not(any(windows, target_os = "linux")))]
mod platform {
    use super::Entry;
    use std::io;

    pub(super) fn read_entries() -> io::Result<Vec<Entry>> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "neighbor-table lookup is supported only on Windows and Linux",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_matching_mac_and_deduplicates_same_address() {
        let mac = [0x02, 0, 0, 0, 0, 0xab];
        let entries = [
            Entry {
                address: Ipv4Addr::new(192, 0, 2, 10),
                mac,
            },
            Entry {
                address: Ipv4Addr::new(192, 0, 2, 10),
                mac,
            },
            Entry {
                address: Ipv4Addr::new(192, 0, 2, 11),
                mac: [0; 6],
            },
        ];
        let matches = matching_addresses(entries, mac);
        assert_eq!(matches, [Ipv4Addr::new(192, 0, 2, 10)]);
    }
}
