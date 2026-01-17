//! Command help entries

use super::{HelpSystem, command_help};

pub fn register_help(system: &mut HelpSystem) {
    // File system commands
    system.add_entry(command_help(
        "ls",
        "List directory contents",
        "Display files and directories in the specified path or current directory.\n\
         Shows file names, sizes, permissions, and modification times.",
        "ls [OPTIONS] [PATH]",
        &[
            "ls           - List current directory",
            "ls /home     - List /home directory",
            "ls -l        - Long format with details",
            "ls -a        - Show hidden files",
            "ls -la /etc  - Long format, hidden files in /etc",
        ],
        &["cd", "pwd", "mkdir"],
    ));

    system.add_entry(command_help(
        "cd",
        "Change directory",
        "Change the current working directory to the specified path.\n\
         Use '..' to go up one level, '/' for root, '~' for home.",
        "cd [PATH]",
        &[
            "cd /home     - Go to /home",
            "cd ..        - Go up one directory",
            "cd ~         - Go to home directory",
            "cd           - Go to home directory",
        ],
        &["ls", "pwd"],
    ));

    system.add_entry(command_help(
        "pwd",
        "Print working directory",
        "Display the full path of the current working directory.",
        "pwd",
        &["pwd"],
        &["cd", "ls"],
    ));

    system.add_entry(command_help(
        "mkdir",
        "Make directory",
        "Create new directories.",
        "mkdir [OPTIONS] DIRECTORY...",
        &[
            "mkdir mydir       - Create 'mydir'",
            "mkdir -p a/b/c    - Create nested directories",
        ],
        &["rmdir", "ls"],
    ));

    system.add_entry(command_help(
        "rm",
        "Remove files or directories",
        "Delete files and directories from the filesystem.",
        "rm [OPTIONS] FILE...",
        &[
            "rm file.txt       - Remove a file",
            "rm -r directory   - Remove directory recursively",
            "rm -f file        - Force remove without prompting",
        ],
        &["rmdir", "mv"],
    ));

    system.add_entry(command_help(
        "cp",
        "Copy files and directories",
        "Copy files from source to destination.",
        "cp [OPTIONS] SOURCE DEST",
        &[
            "cp file1 file2    - Copy file1 to file2",
            "cp -r dir1 dir2   - Copy directory recursively",
        ],
        &["mv", "rm"],
    ));

    system.add_entry(command_help(
        "mv",
        "Move or rename files",
        "Move files to a new location or rename them.",
        "mv SOURCE DEST",
        &[
            "mv old.txt new.txt    - Rename file",
            "mv file /other/dir    - Move file",
        ],
        &["cp", "rm"],
    ));

    system.add_entry(command_help(
        "cat",
        "Display file contents",
        "Concatenate and display file contents to standard output.",
        "cat FILE...",
        &[
            "cat file.txt      - Display file contents",
            "cat f1 f2         - Display multiple files",
        ],
        &["less", "head", "tail"],
    ));

    system.add_entry(command_help(
        "touch",
        "Create empty file or update timestamp",
        "Create a new empty file or update the access/modification time of an existing file.",
        "touch FILE...",
        &[
            "touch newfile.txt - Create empty file",
        ],
        &["mkdir", "rm"],
    ));

    // Process commands
    system.add_entry(command_help(
        "ps",
        "List processes",
        "Display information about running processes.",
        "ps [OPTIONS]",
        &[
            "ps        - List user processes",
            "ps -a     - List all processes",
            "ps -aux   - Detailed process info",
        ],
        &["kill", "top"],
    ));

    system.add_entry(command_help(
        "kill",
        "Send signal to process",
        "Send a signal to a process by PID. Default signal is SIGTERM.",
        "kill [SIGNAL] PID",
        &[
            "kill 1234      - Send SIGTERM to PID 1234",
            "kill -9 1234   - Send SIGKILL (force kill)",
            "kill -SIGINT 1234",
        ],
        &["ps", "top"],
    ));

    // System commands
    system.add_entry(command_help(
        "shutdown",
        "Shut down the system",
        "Safely shut down the system.",
        "shutdown [OPTIONS]",
        &[
            "shutdown       - Shutdown immediately",
            "shutdown -r    - Reboot instead",
        ],
        &["reboot"],
    ));

    system.add_entry(command_help(
        "reboot",
        "Restart the system",
        "Safely restart the system.",
        "reboot",
        &["reboot"],
        &["shutdown"],
    ));

    // User commands
    system.add_entry(command_help(
        "whoami",
        "Print current user",
        "Display the username of the current user.",
        "whoami",
        &["whoami"],
        &["id", "su"],
    ));

    system.add_entry(command_help(
        "su",
        "Switch user",
        "Switch to another user account.",
        "su [USER]",
        &[
            "su           - Switch to root",
            "su admin     - Switch to admin user",
        ],
        &["sudo", "whoami"],
    ));

    system.add_entry(command_help(
        "sudo",
        "Execute as superuser",
        "Execute a command with superuser privileges.",
        "sudo COMMAND",
        &[
            "sudo ls /root        - List /root as root",
            "sudo vim /etc/fstab  - Edit system file",
        ],
        &["su", "whoami"],
    ));

    // Network commands
    system.add_entry(command_help(
        "ping",
        "Send ICMP echo request",
        "Send ICMP echo requests to a host to test network connectivity.",
        "ping [OPTIONS] HOST",
        &[
            "ping google.com      - Ping google.com",
            "ping -c 4 192.168.1.1 - Send 4 pings",
        ],
        &["ifconfig", "netstat"],
    ));

    system.add_entry(command_help(
        "ifconfig",
        "Configure network interface",
        "Display or configure network interfaces.",
        "ifconfig [INTERFACE] [OPTIONS]",
        &[
            "ifconfig         - Show all interfaces",
            "ifconfig eth0    - Show eth0 details",
        ],
        &["ping", "netstat"],
    ));

    // Misc commands
    system.add_entry(command_help(
        "echo",
        "Display text",
        "Print arguments to standard output.",
        "echo [TEXT...]",
        &[
            "echo Hello World",
            "echo $HOME",
        ],
        &["cat", "printf"],
    ));

    system.add_entry(command_help(
        "clear",
        "Clear screen",
        "Clear the terminal screen.",
        "clear",
        &["clear"],
        &[],
    ));

    system.add_entry(command_help(
        "help",
        "Display help",
        "Display help information about commands and topics.",
        "help [TOPIC]",
        &[
            "help         - Show overview",
            "help ls      - Help for ls command",
            "help files   - Help on file concepts",
        ],
        &[],
    ));

    system.add_entry(command_help(
        "exit",
        "Exit shell",
        "Exit the current shell session.",
        "exit [CODE]",
        &[
            "exit         - Exit with code 0",
            "exit 1       - Exit with code 1",
        ],
        &["logout"],
    ));
}
