from django.contrib import admin
from django.urls import include, path
from rest_framework import routers
from rest_framework.schemas import get_schema_view

router = routers.DefaultRouter(trailing_slash=False)


urlpatterns = [
    path("admin/", admin.site.urls),
    path("api/v1/", include("hpcc_api.urls")),
]
