FROM python:3.11-slim as requirements-stage

RUN apt-get update \
    && apt-get install --no-install-recommends -y \
    build-essential \
    curl \
    gcc \
    libffi-dev \
    libpq-dev

WORKDIR /tmp

ENV PIP_DEFAULT_TIMEOUT=100 \
    PIP_DISABLE_PIP_VERSION_CHECK=1 \
    PIP_NO_CACHE_DIR=1 \
    POETRY_VERSION=1.4.2

RUN pip install "poetry==$POETRY_VERSION"

COPY ./pyproject.toml ./poetry.lock* /tmp/

RUN poetry export -f requirements.txt --output requirements.txt --without-hashes


FROM python:3.11-slim as run-stage

WORKDIR /code

RUN apt update -y && \
    apt install -y libpq-dev build-essential

ENV PYTHONFAULTHANDLER=1 \
    PYTHONHASHSEED=random \
    PYTHONUNBUFFERED=1

COPY --from=requirements-stage /tmp/requirements.txt /code/requirements.txt

RUN pip install --no-cache-dir --upgrade -r /code/requirements.txt

COPY ./hpcatcloud/ /code/hpcatcloud/

COPY ./manage.py /code/manage.py

ENV WORKERS=2

EXPOSE 8000

CMD gunicorn --bind 0.0.0.0:8000 hpcatcloud.wsgi --workers=$WORKERS --access-logfile -
