from socket import timeout
import select
import time

import paramiko
from paramiko.ssh_exception import NoValidConnectionsError, SSHException
from scp import SCPClient

from hpcac_cli.utils.logger import Logger


log = Logger()


def ping(ip: str, username: str) -> bool:
    ssh = paramiko.SSHClient()
    ssh.set_missing_host_key_policy(paramiko.AutoAddPolicy())

    is_healthy = False
    try:
        ssh.connect(ip, username=username, timeout=3)
        # Run a simple command like 'echo' to check the connection
        _stdin, _stdout, _stderr = ssh.exec_command('echo "I\'m alive!"')
        is_healthy = True
    except (NoValidConnectionsError, SSHException, timeout) as e:
        log.warning(f"Node `{ip}` unreachable...")
    finally:
        ssh.close()

    return is_healthy


def remote_command(ip: str, username: str, command: str) -> bool:
    ssh = paramiko.SSHClient()
    ssh.set_missing_host_key_policy(paramiko.AutoAddPolicy())
    success = False

    try:
        log.debug(text=f"Running command: `{command}`", detail=ip)
        ssh.connect(ip, username=username, timeout=300)
        _stdin, stdout, stderr = ssh.exec_command(command)

        # Continuously read and print stdout as it becomes available
        while not stdout.channel.exit_status_ready():
            if stdout.channel.recv_ready():
                rl, _, _ = select.select([stdout.channel], [], [], 0.0)
                if rl:
                    print(stdout.channel.recv(1024).decode("utf-8"), end="")
            else:
                log.debug(f"Waiting for remote command stdout, stderr...")
                time.sleep(0.1)

        _stdout_text = stdout.read().decode().strip()
        stderr_text = stderr.read().decode().strip()

        exit_status = stdout.channel.recv_exit_status()
        if exit_status == 0:
            success = True
            if stderr_text:
                log.warning(
                    f"STDERR: ```\n{stderr_text}\n``` while running remote command `{command}` at Node: `{ip}`"
                )
                if "PRTE has lost communication with a remote daemon" in stderr_text:
                    success = False
        else:
            if (
                "unreachable: [Errno None] Unable to connect" in stderr_text
                or "PRTE has lost communication with a remote daemon" in stderr_text
            ):
                log.warning(
                    f"STDERR: ```\n{stderr_text}\n``` while running remote command `{command}` at Node: `{ip}`"
                )
            else:
                log.error(
                    f"STDERR: ```\n{stderr_text}\n``` while running remote command `{command}` at Node: `{ip}`"
                )

    except Exception as e:
        log.error(
            f"EXCEPTION: ```\n{e}\n``` while running remote command `{command}` at Node: `{ip}"
        )
    finally:
        ssh.close()

    return success


def scp_transfer_directory(local_path: str, remote_path: str, ip: str, username: str):
    ssh = paramiko.SSHClient()
    ssh.set_missing_host_key_policy(paramiko.AutoAddPolicy())
    try:
        ssh.connect(ip, username=username, timeout=3)
        with SCPClient(ssh.get_transport()) as scp:
            scp.put(local_path, remote_path, recursive=True)
            log.debug(
                text=f"Directory `{local_path}` transferred to `{remote_path}`",
                detail=ip,
            )
    except Exception as e:
        log.error(
            f"EXCEPTION: ```\n{e}\n``` while transfering directory `{local_path}` to `{ip}`:`{remote_path}`"
        )
    finally:
        ssh.close()


def scp_download_directory(remote_path: str, local_path: str, ip: str, username: str):
    ssh = paramiko.SSHClient()
    ssh.set_missing_host_key_policy(paramiko.AutoAddPolicy())
    try:
        ssh.connect(ip, username=username, timeout=3)
        with SCPClient(ssh.get_transport()) as scp:
            scp.get(remote_path, local_path, recursive=True)
            log.debug(
                text=f"Directory `{remote_path}` downloaded to `{local_path}`",
                detail=ip,
            )
    except Exception as e:
        log.error(
            f"EXCEPTION: ```\n{e}\n``` while downloading directory `{remote_path}` from `{ip}` to `{local_path}`"
        )
    finally:
        ssh.close()
