from socket import timeout

from scp import SCPClient
import paramiko
from paramiko.ssh_exception import NoValidConnectionsError, SSHException

from hpcac_cli.utils.logger import info_remote, error


def ping(ip: str, username: str) -> bool:
    ssh = paramiko.SSHClient()
    ssh.set_missing_host_key_policy(paramiko.AutoAddPolicy())

    is_healthy = False
    try:
        ssh.connect(ip, username=username, timeout=3)
        # Run a simple command like 'echo' to check the connection
        stdin, stdout, stderr = ssh.exec_command('echo "I\'m alive!"')
        info_remote(ip=ip, text=f"{stdout.read().decode().strip()}")
        is_healthy = True
    except (NoValidConnectionsError, SSHException, timeout) as e:
        error(f"Node `{ip}` unreachable: {e}")

    finally:
        ssh.close()

    return is_healthy


def remote_command(ip: str, username: str, command: str):
    ssh = paramiko.SSHClient()
    ssh.set_missing_host_key_policy(paramiko.AutoAddPolicy())
    try:
        info_remote(ip=ip, text=f"Running command: `{command}`")
        ssh.connect(ip, username=username, timeout=3)
        stdin, stdout, stderr = ssh.exec_command(command)
        text = f"{stdout.read().decode().strip()}"
        if text != "":
            info_remote(ip=ip, text=text)
        
        stderr_text = f"{stderr.read().decode().strip()}"
        if stderr_text != "":
            raise Exception(stderr_text)
    except (Exception) as e:
        error(
            f"Exception: `{e}` running remote command `{command}` at Node: `{ip}`"
        )
    finally:
        ssh.close()


def scp_transfer_file(local_path: str, remote_path: str, ip: str, username: str):
    ssh = paramiko.SSHClient()
    ssh.set_missing_host_key_policy(paramiko.AutoAddPolicy())
    try:
        ssh.connect(ip, username=username, timeout=3)
        with SCPClient(ssh.get_transport()) as scp:
            scp.put(local_path, remote_path)
            info_remote(ip=ip, text=f"File `{local_path}` transferred to `{remote_path}`")
    except Exception as e:
        error(
            f"Exception: `{e}` transfering `{local_path}` to `{ip}`:`{remote_path}`"
        )
    finally:
        ssh.close()
