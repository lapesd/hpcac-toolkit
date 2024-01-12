import os

from minio import Minio

from hpcac_cli.utils.logger import info


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
    info(
        f"Created `{minio_response.object_name}` object with etag: `{minio_response.etag}` "
        f"in MinIO bucket: `{bucket_name}`"
    )


def download_file_from_minio_bucket(file_path: str, object_name: str, bucket_name: str):
    minio_response = minio.fget_object(
        bucket_name=bucket_name,
        object_name=object_name,
        file_path=file_path,
    )
    info(
        f"Downloaded `{minio_response.object_name}` object with etag: `{minio_response.etag}` "
        f"in MinIO bucket: `{bucket_name}`"
    )
