import socket
import time

def main():
    server_socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    server_socket.bind(('0.0.0.0', 12345))
    server_socket.listen(1)
    print("Server is listening on port 12345...")

    conn, addr = server_socket.accept()
    print("Connected to:", addr)

    start_time = time.time()

    data_received = 0
    buffer_size = 1024
    while True:
        data = conn.recv(buffer_size)
        if not data:
            break
        data_received += len(data)

    end_time = time.time()
    duration = end_time - start_time

    print(f"Data received: {data_received / (1024 * 1024)} MB")
    print(f"Time taken: {duration} seconds")
    print(f"Bandwidth: {data_received / (1024 * 1024) / duration} MB/s")

    conn.close()
    server_socket.close()

if __name__ == "__main__":
    main()
