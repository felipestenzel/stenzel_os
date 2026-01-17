//! User Manual entries

use super::{HelpSystem, HelpEntry, HelpCategory};
use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;

pub fn register_help(system: &mut HelpSystem) {
    // Getting Started
    system.add_entry(HelpEntry {
        name: String::from("getting-started"),
        category: HelpCategory::Tutorial,
        short_desc: String::from("Introduction to Stenzel OS"),
        long_desc: String::from(
"Welcome to Stenzel OS!

This is a modern operating system written in Rust from scratch.
It provides a familiar Unix-like environment with a graphical
desktop and full networking support.

FIRST STEPS
-----------
1. Log in with your username and password
2. The desktop will appear with a taskbar at the bottom
3. Click the Start Menu or use keyboard shortcuts
4. Open a terminal for command-line access

BASIC NAVIGATION
----------------
- Click on windows to focus them
- Drag title bars to move windows
- Use the taskbar to switch between applications
- Press Alt+Tab to cycle through windows

GETTING HELP
------------
- Type 'help' in the terminal for command help
- Use 'help <topic>' for specific information
- Press F1 in most applications for help

For more information, see: filesystem, desktop, network"
        ),
        usage: None,
        examples: Vec::new(),
        see_also: vec![
            String::from("filesystem"),
            String::from("desktop"),
            String::from("network"),
        ],
    });

    // Filesystem guide
    system.add_entry(HelpEntry {
        name: String::from("filesystem"),
        category: HelpCategory::Concept,
        short_desc: String::from("Understanding the filesystem"),
        long_desc: String::from(
"STENZEL OS FILESYSTEM
=====================

Stenzel OS uses a hierarchical filesystem with / as the root.

DIRECTORY STRUCTURE
-------------------
/           Root directory
/bin        Essential command binaries
/boot       Boot loader files
/dev        Device files
/etc        System configuration
/home       User home directories
/lib        Essential shared libraries
/mnt        Mount point for temporary mounts
/proc       Process information (virtual)
/root       Root user's home directory
/sys        System information (virtual)
/tmp        Temporary files
/usr        User programs and data
/var        Variable data (logs, spool, etc.)

FILE TYPES
----------
-  Regular file
d  Directory
l  Symbolic link
b  Block device
c  Character device
p  Named pipe (FIFO)
s  Socket

PERMISSIONS
-----------
Files have three permission sets: owner, group, others.
Each set has read (r), write (w), and execute (x) flags.

Example: -rwxr-xr-x
  Owner: rwx (read, write, execute)
  Group: r-x (read, execute)
  Other: r-x (read, execute)

Use 'chmod' to change permissions, 'chown' to change owner.

SUPPORTED FILESYSTEMS
---------------------
- ext2/ext4 (Linux native)
- FAT32 (USB drives)
- tmpfs (in-memory)
- procfs (/proc)
- sysfs (/sys)
- devfs (/dev)"
        ),
        usage: None,
        examples: Vec::new(),
        see_also: vec![
            String::from("ls"),
            String::from("cd"),
            String::from("chmod"),
        ],
    });

    // Desktop guide
    system.add_entry(HelpEntry {
        name: String::from("desktop"),
        category: HelpCategory::Concept,
        short_desc: String::from("Using the graphical desktop"),
        long_desc: String::from(
"STENZEL OS DESKTOP ENVIRONMENT
==============================

The desktop provides a graphical user interface with windows,
menus, and familiar UI elements.

COMPONENTS
----------
Desktop     - Main screen area with wallpaper and icons
Taskbar     - Bottom panel with Start Menu and running apps
Start Menu  - Access to all applications
System Tray - Network, volume, battery indicators

WINDOW MANAGEMENT
-----------------
- Click and drag title bar to move windows
- Drag window edges to resize
- Close button (X) to close window
- Minimize (-) to hide to taskbar
- Maximize (□) to fill screen

KEYBOARD SHORTCUTS
------------------
Alt+Tab         Cycle through windows
Alt+F4          Close current window
Ctrl+Alt+T      Open terminal
Ctrl+Alt+Del    System menu
Win/Super       Open Start Menu
F11             Toggle fullscreen

TASKBAR
-------
- Click Start Menu for applications
- Click running app to switch to it
- Right-click for context menu
- System tray shows status indicators

MULTIPLE DESKTOPS
-----------------
Use Ctrl+Alt+Left/Right to switch between virtual desktops.
Each desktop can have its own set of windows.

SETTINGS
--------
Open Settings app to customize:
- Display resolution and brightness
- Network connections
- Date and time
- Keyboard layout
- User accounts"
        ),
        usage: None,
        examples: Vec::new(),
        see_also: vec![
            String::from("settings"),
            String::from("terminal"),
        ],
    });

    // Network guide
    system.add_entry(HelpEntry {
        name: String::from("network"),
        category: HelpCategory::Concept,
        short_desc: String::from("Network configuration and usage"),
        long_desc: String::from(
"STENZEL OS NETWORKING
=====================

Stenzel OS supports both wired and wireless networking.

ETHERNET (WIRED)
----------------
Ethernet connections are usually automatic via DHCP.
The system will obtain an IP address automatically when
a cable is connected.

To manually configure:
  ifconfig eth0 192.168.1.100 netmask 255.255.255.0
  route add default gw 192.168.1.1

WIFI (WIRELESS)
---------------
1. Click the network icon in the system tray
2. Select a wireless network from the list
3. Enter the password if required
4. The connection will be saved for future use

Command line:
  wifi scan              - List available networks
  wifi connect SSID      - Connect to network
  wifi status            - Show connection status

DNS CONFIGURATION
-----------------
DNS servers are usually obtained via DHCP.
Manual configuration in /etc/resolv.conf:
  nameserver 8.8.8.8
  nameserver 8.8.4.4

USEFUL COMMANDS
---------------
ping host       - Test connectivity
ifconfig        - Show network interfaces
netstat         - Show connections
route           - Show routing table
nslookup host   - DNS lookup

TROUBLESHOOTING
---------------
1. Check if interface is up: ifconfig
2. Try pinging gateway: ping 192.168.1.1
3. Try pinging external: ping 8.8.8.8
4. Check DNS: nslookup google.com

If DHCP fails, try: dhclient eth0"
        ),
        usage: None,
        examples: Vec::new(),
        see_also: vec![
            String::from("ping"),
            String::from("ifconfig"),
            String::from("wifi"),
        ],
    });

    // Security guide
    system.add_entry(HelpEntry {
        name: String::from("security"),
        category: HelpCategory::Concept,
        short_desc: String::from("System security and permissions"),
        long_desc: String::from(
"STENZEL OS SECURITY
===================

USER ACCOUNTS
-------------
Each user has:
- Username and UID (User ID)
- Primary group and GID (Group ID)
- Home directory
- Login shell

Create users with: useradd username
Delete users with: userdel username
Change password with: passwd

ROOT AND SUDO
-------------
The root account has full system access.
Regular users can run commands as root using sudo:
  sudo command

Users must be in the 'wheel' group for sudo access.

FILE PERMISSIONS
----------------
Every file has owner, group, and permissions.
Use chmod to change: chmod 755 file
Use chown to change owner: chown user:group file

Permission numbers:
  4 = read
  2 = write
  1 = execute

Example: 755 = rwxr-xr-x

BEST PRACTICES
--------------
1. Use strong passwords
2. Don't run as root normally
3. Keep system updated
4. Check file permissions
5. Review /var/log for issues"
        ),
        usage: None,
        examples: Vec::new(),
        see_also: vec![
            String::from("chmod"),
            String::from("sudo"),
            String::from("passwd"),
        ],
    });

    // Package management guide
    system.add_entry(HelpEntry {
        name: String::from("packages"),
        category: HelpCategory::Concept,
        short_desc: String::from("Installing and managing software"),
        long_desc: String::from(
"PACKAGE MANAGEMENT
==================

Stenzel OS uses .spkg packages for software distribution.

BASIC COMMANDS
--------------
pkg install <name>   - Install a package
pkg remove <name>    - Remove a package
pkg update           - Update package lists
pkg upgrade          - Upgrade all packages
pkg search <term>    - Search for packages
pkg info <name>      - Show package info

REPOSITORIES
------------
Packages are downloaded from configured repositories.
Repository configuration is in /etc/pkg/repos.conf

DEPENDENCIES
------------
When you install a package, its dependencies are
automatically installed as well. When you remove a
package, orphaned dependencies can be cleaned up.

MANUAL INSTALLATION
-------------------
To install a downloaded .spkg file:
  pkg install -f package.spkg

PACKAGE BUILDING
----------------
Build packages from source using build recipes.
Recipes are stored in /usr/src/recipes/.

VERIFYING PACKAGES
------------------
All packages are cryptographically signed.
Signatures are verified automatically during install."
        ),
        usage: None,
        examples: Vec::new(),
        see_also: vec![
            String::from("pkg"),
        ],
    });

    // Troubleshooting guide
    system.add_entry(HelpEntry {
        name: String::from("troubleshooting"),
        category: HelpCategory::Faq,
        short_desc: String::from("Common problems and solutions"),
        long_desc: String::from(
"TROUBLESHOOTING GUIDE
=====================

BOOT PROBLEMS
-------------
Q: System won't boot
A: - Check BIOS boot order
   - Verify boot media is inserted
   - Try recovery mode (hold Shift during boot)

Q: Black screen after boot
A: - Try pressing Ctrl+Alt+F2 for console
   - Check video driver settings

DISPLAY ISSUES
--------------
Q: Wrong resolution
A: Settings > Display > Resolution
   Or edit /etc/X11/xorg.conf

Q: Screen flickering
A: Try different refresh rate in Display settings

NETWORK PROBLEMS
----------------
Q: No network connection
A: - Check cable/wifi connection
   - Run 'ifconfig' to see interface status
   - Try 'dhclient eth0' for DHCP

Q: Can ping IP but not hostname
A: DNS issue - check /etc/resolv.conf

PERFORMANCE
-----------
Q: System is slow
A: - Check CPU/memory in Task Manager
   - Close unused applications
   - Check disk space with 'df -h'

Q: Running out of memory
A: - Close applications
   - Check for memory leaks with 'top'

LOGIN ISSUES
------------
Q: Forgot password
A: Boot recovery mode and use 'passwd' command

Q: Account locked
A: Root can unlock with: passwd -u username

GETTING LOGS
------------
System logs are in /var/log/
- /var/log/syslog - General system log
- /var/log/kern.log - Kernel messages
- dmesg - Kernel ring buffer"
        ),
        usage: None,
        examples: Vec::new(),
        see_also: vec![
            String::from("dmesg"),
            String::from("journalctl"),
        ],
    });
}
