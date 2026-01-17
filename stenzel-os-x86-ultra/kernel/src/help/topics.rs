//! Help topics for concepts and tutorials

use super::{HelpSystem, HelpEntry, HelpCategory};
use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;

pub fn register_help(system: &mut HelpSystem) {
    // Process management
    system.add_entry(HelpEntry {
        name: String::from("processes"),
        category: HelpCategory::Concept,
        short_desc: String::from("Understanding processes"),
        long_desc: String::from(
"PROCESS MANAGEMENT
==================

A process is a running instance of a program.

PROCESS STATES
--------------
Running   - Currently executing on CPU
Ready     - Waiting for CPU time
Sleeping  - Waiting for I/O or event
Stopped   - Suspended (e.g., Ctrl+Z)
Zombie    - Terminated, waiting for parent

PROCESS INFORMATION
-------------------
PID       - Process ID (unique number)
PPID      - Parent Process ID
UID       - User ID that owns the process
Priority  - Scheduling priority
Memory    - Memory usage

VIEWING PROCESSES
-----------------
ps        - List processes
top       - Interactive process viewer
htop      - Enhanced process viewer

CONTROLLING PROCESSES
---------------------
Ctrl+C    - Send SIGINT (interrupt)
Ctrl+Z    - Send SIGSTOP (suspend)
fg        - Resume in foreground
bg        - Resume in background
kill PID  - Send signal to process

SIGNALS
-------
SIGINT   (2)  - Interrupt (Ctrl+C)
SIGKILL  (9)  - Force kill (cannot be caught)
SIGTERM (15)  - Terminate gracefully
SIGSTOP (19)  - Stop process
SIGCONT (18)  - Continue stopped process"
        ),
        usage: None,
        examples: Vec::new(),
        see_also: vec![String::from("ps"), String::from("kill"), String::from("top")],
    });

    // Shell usage
    system.add_entry(HelpEntry {
        name: String::from("shell"),
        category: HelpCategory::Concept,
        short_desc: String::from("Using the command shell"),
        long_desc: String::from(
"SHELL USAGE
===========

The shell is a command-line interface for interacting with the system.

BASIC SYNTAX
------------
command [options] [arguments]

Options usually start with - or --
  ls -l          Short option
  ls --long      Long option

SPECIAL CHARACTERS
------------------
|    Pipe: send output to next command
>    Redirect output to file (overwrite)
>>   Redirect output to file (append)
<    Redirect input from file
&    Run command in background
;    Separate commands
&&   Run next command only if previous succeeded
||   Run next command only if previous failed

ENVIRONMENT VARIABLES
---------------------
$HOME   - Home directory
$PATH   - Command search path
$USER   - Current username
$PWD    - Current directory

Set variable: export VAR=value
Use variable: echo $VAR

COMMAND HISTORY
---------------
Up/Down arrows - Navigate history
Ctrl+R        - Search history
!!            - Repeat last command
!n            - Repeat command n

TAB COMPLETION
--------------
Press Tab to complete:
- Commands
- File paths
- Options (in some shells)

JOB CONTROL
-----------
Ctrl+Z        - Suspend current job
jobs          - List jobs
fg %n         - Bring job n to foreground
bg %n         - Resume job n in background"
        ),
        usage: None,
        examples: Vec::new(),
        see_also: vec![String::from("bash"), String::from("environment")],
    });

    // Keyboard shortcuts
    system.add_entry(HelpEntry {
        name: String::from("shortcuts"),
        category: HelpCategory::Concept,
        short_desc: String::from("Keyboard shortcuts reference"),
        long_desc: String::from(
"KEYBOARD SHORTCUTS
==================

DESKTOP SHORTCUTS
-----------------
Alt+Tab         Switch windows
Alt+F4          Close window
Super/Win       Open Start Menu
Ctrl+Alt+T      Open terminal
Ctrl+Alt+Del    System menu
F11             Toggle fullscreen
Print Screen    Take screenshot

TERMINAL SHORTCUTS
------------------
Ctrl+C          Interrupt process
Ctrl+Z          Suspend process
Ctrl+D          End of input / logout
Ctrl+L          Clear screen
Ctrl+A          Move to line start
Ctrl+E          Move to line end
Ctrl+K          Delete to end of line
Ctrl+U          Delete to start of line
Ctrl+W          Delete previous word
Ctrl+R          Search history
Tab             Auto-complete

TEXT EDITING
------------
Ctrl+C          Copy
Ctrl+V          Paste
Ctrl+X          Cut
Ctrl+Z          Undo
Ctrl+Y          Redo
Ctrl+A          Select all
Ctrl+S          Save
Ctrl+F          Find
Ctrl+H          Replace

NAVIGATION
----------
Alt+Left        Go back
Alt+Right       Go forward
Alt+Up          Go to parent folder
Home            Go to start
End             Go to end
Ctrl+Home       Go to beginning
Ctrl+End        Go to end

FILE MANAGER
------------
Enter           Open selected
Delete          Delete selected
F2              Rename
Ctrl+N          New window
Ctrl+H          Toggle hidden files"
        ),
        usage: None,
        examples: Vec::new(),
        see_also: vec![String::from("desktop"), String::from("shell")],
    });

    // Configuration files
    system.add_entry(HelpEntry {
        name: String::from("config"),
        category: HelpCategory::Concept,
        short_desc: String::from("Configuration files reference"),
        long_desc: String::from(
"CONFIGURATION FILES
===================

SYSTEM CONFIGURATION
--------------------
/etc/hostname        System hostname
/etc/hosts           Host name resolution
/etc/fstab           Filesystem mount table
/etc/resolv.conf     DNS resolver config
/etc/passwd          User accounts
/etc/group           Group definitions
/etc/shadow          Password hashes (restricted)
/etc/sudoers         Sudo configuration

NETWORK CONFIGURATION
---------------------
/etc/network/interfaces    Network interface config
/etc/wpa_supplicant.conf   WiFi configuration
/etc/dhclient.conf         DHCP client config

USER CONFIGURATION
------------------
~/.profile           Login shell config
~/.bashrc            Bash interactive config
~/.config/           User app settings
~/.local/            User data and binaries

PACKAGE MANAGEMENT
------------------
/etc/pkg/repos.conf  Repository configuration
/var/lib/pkg/db      Package database

BOOT CONFIGURATION
------------------
/boot/grub/grub.cfg  GRUB bootloader config
/etc/default/grub    GRUB defaults

LOG FILES
---------
/var/log/syslog      System messages
/var/log/kern.log    Kernel messages
/var/log/auth.log    Authentication log
/var/log/dmesg       Boot messages"
        ),
        usage: None,
        examples: Vec::new(),
        see_also: vec![String::from("filesystem"), String::from("security")],
    });

    // System administration
    system.add_entry(HelpEntry {
        name: String::from("admin"),
        category: HelpCategory::Tutorial,
        short_desc: String::from("System administration guide"),
        long_desc: String::from(
"SYSTEM ADMINISTRATION
=====================

USER MANAGEMENT
---------------
useradd user    Create new user
userdel user    Delete user
passwd user     Change password
usermod         Modify user account
groupadd        Create group
groups user     Show user's groups

DISK MANAGEMENT
---------------
df -h           Show disk space
du -sh dir      Show directory size
mount           Mount filesystem
umount          Unmount filesystem
fsck            Check filesystem
mkfs            Create filesystem

SYSTEM MONITORING
-----------------
top             Process monitor
free -h         Memory usage
uptime          System uptime
vmstat          Virtual memory stats
iostat          I/O statistics

SERVICE MANAGEMENT
------------------
systemctl start service
systemctl stop service
systemctl restart service
systemctl status service
systemctl enable service

SCHEDULED TASKS
---------------
crontab -e      Edit user cron jobs
/etc/cron.d/    System cron jobs

LOGS
----
journalctl      View system logs
dmesg           Kernel messages
tail -f /var/log/syslog

BACKUP
------
cp -a           Copy with attributes
tar -cvzf       Create compressed archive
rsync           Synchronize directories"
        ),
        usage: None,
        examples: Vec::new(),
        see_also: vec![String::from("security"), String::from("filesystem")],
    });

    // FAQ
    system.add_entry(HelpEntry {
        name: String::from("faq"),
        category: HelpCategory::Faq,
        short_desc: String::from("Frequently asked questions"),
        long_desc: String::from(
"FREQUENTLY ASKED QUESTIONS
==========================

Q: How do I change my password?
A: Run 'passwd' and follow the prompts.

Q: How do I become root?
A: Use 'sudo -i' or 'su -' if you have permissions.

Q: How do I install software?
A: Use 'pkg install <package>' or the Software Center app.

Q: Where are my files?
A: User files are in /home/username/
   Desktop files are in /home/username/Desktop/

Q: How do I connect to WiFi?
A: Click network icon in system tray, select network,
   enter password. Or use 'wifi connect SSID' command.

Q: How do I open a terminal?
A: Press Ctrl+Alt+T or find Terminal in Start Menu.

Q: How do I shut down?
A: Click Start Menu > Power > Shutdown
   Or run 'shutdown' in terminal.

Q: How do I find a file?
A: Use 'find /path -name filename' or
   'locate filename' if database is built.

Q: How do I see system information?
A: Settings > About, or run 'uname -a', 'lscpu', 'free -h'

Q: Where are system logs?
A: In /var/log/ directory. Use 'dmesg' for kernel messages.

Q: How do I mount a USB drive?
A: Usually automatic. Manual: 'mount /dev/sdb1 /mnt'

Q: How do I update the system?
A: Run 'pkg update && pkg upgrade' or use Software Center."
        ),
        usage: None,
        examples: Vec::new(),
        see_also: vec![String::from("getting-started"), String::from("troubleshooting")],
    });
}
