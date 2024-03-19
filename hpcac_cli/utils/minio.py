import os

from minio import Minio

from hpcac_cli.utils.logger import Logger


log = Logger()


minio = Minio(
    "localhost:9000",
    access_key="root",
    secret_key="password",
    secure=False,
)


def create_minio_bucket(bucket_name: str):
    if not minio.bucket_exists(bucket_name):
        minio.make_bucket(bucket_name)


def upload_file_to_minio_bucket(file_path: str, object_name: str, bucket_name: str):
    minio_response = minio.fput_object(
        bucket_name=bucket_name,
        object_name=object_name,
        file_path=os.path.abspath(file_path),
    )
    log.debug(
        text=f"Created `{minio_response.object_name}` object with etag: `{minio_response.etag}` in MinIO bucket: `{bucket_name}`",
        detail="upload_file_to_minio_bucket",
    )


def download_file_from_minio_bucket(file_path: str, object_name: str, bucket_name: str):
    minio_response = minio.fget_object(
        bucket_name=bucket_name,
        object_name=object_name,
        file_path=file_path,
    )
    log.debug(
        text=f"Downloaded `{minio_response.object_name}` object with etag: `{minio_response.etag}` from MinIO bucket: `{bucket_name}`",
        detail="download_file_from_minio_bucket",
    )
