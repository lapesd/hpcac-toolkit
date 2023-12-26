from django.contrib import admin
from django.urls import include, path
from rest_framework import routers

from hpcatcloud.clusters import views as clusters_views


router = routers.DefaultRouter(trailing_slash=False)

router.register(r"clusters", clusters_views.ClusterViewSet)

urlpatterns = [
    path("admin/", admin.site.urls),
    path("api/v1/", include(router.urls)),
]
