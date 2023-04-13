import socket
import time
import sys

def main():
    if len(sys.argv) < 2:
        print("Usage: python client.py <server_ip>")
        sys.exit(1)

    server_ip = sys.argv[1]

    client_socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    client_socket.connect((server_ip, 12345))
    print("Connected to server.")

    data_size_mb = 100  # Adjust this value to send more or less data
    data_size = data_size_mb * 1024 * 1024
    buffer_size = 1024
    data = b'0' * buffer_size

    start_time = time.time()

    for _ in range(data_size // buffer_size):
        client_socket.send(data)

    end_time = time.time()
    duration = end_time - start_time

    print(f"Data sent: {data_size_mb} MB")
    print(f"Time taken: {duration} seconds")
    print(f"Bandwidth: {data_size_mb / duration} MB/s")

    client_socket.close()

if __name__ == "__main__":
    main()
