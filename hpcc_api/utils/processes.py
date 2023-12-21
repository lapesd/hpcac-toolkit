import subprocess


def launch_over_ssh(
    remote_command: str,
    ip: str,
    user: str,
    track_output: bool,
) -> int:
    # Set the stdout and stderr based on track_output
    stdout_setting = subprocess.PIPE if track_output else subprocess.DEVNULL

    # Launch process
    process = subprocess.Popen(
        [
            "ssh", "-o", "StrictHostKeyChecking=no",
            f"{user}@{ip}",
            remote_command,
        ],
        stdout=stdout_setting,
        stderr=subprocess.STDOUT,
        text=True,
    )
    
    last_line = ""
    if track_output:
        for line in iter(process.stdout.readline, ""):
            print(line, end="")
            last_line = line if line.strip() != "" else last_line

        process.stdout.close()

    # Wait until completion
    return_code = process.wait()

    # Check for specific error message in output
    if "closed by remote host." in last_line:
        return -1
    elif "--------------------------------------------------------------------------" in last_line:
        return -1

    return return_code
