"""Django URL configuration."""
from django.urls import path, re_path
from . import views


urlpatterns = [
    path("", views.home, name="home"),
    path("about/", views.about, name="about"),
    path("articles/<int:year>/", views.articles_by_year),
    re_path(r"^api/posts/(?P<slug>[\w-]+)/$", views.post_detail),
]
