use cfg_aliases::cfg_aliases;

fn main() {
    cfg_aliases! {
        tcp_transport: {
            any(
                target_os = "linux",
                target_os = "macos",
                target_os = "freebsd",
                target_os = "windows"
            )
        },
        unix_transport: {
            any(
                target_os = "linux",
                target_os = "macos",
                target_os = "freebsd"
            )
        },
        irc_transport: {
            any(
                target_os = "linux",
                target_os = "macos",
                target_os = "freebsd",
                target_os = "windows"
            )
        }
    }
}
