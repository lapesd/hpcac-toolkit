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
