from socket import timeout

import paramiko
from paramiko.ssh_exception import NoValidConnectionsError, SSHException

from hpcac_cli.utils.logger import info, error


def ping(ip: str, username: str) -> bool:
    ssh = paramiko.SSHClient()
    ssh.set_missing_host_key_policy(paramiko.AutoAddPolicy())

    is_healthy = False
    try:
        ssh.connect(ip, username=username, timeout=3)

        # Run a simple command like 'echo' to check the connection
        stdin, stdout, stderr = ssh.exec_command('echo "I\'m alive!"')
        info(f"Node `{ip}` reachable: {stdout.read().decode().strip()}")
        is_healthy = True
    except (NoValidConnectionsError, SSHException, timeout) as e:
        error(f"Node `{ip}` unreachable: {e}")

    finally:
        ssh.close()

    return is_healthy
