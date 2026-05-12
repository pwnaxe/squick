"""URL routing for the marketing site."""
from django.urls import path
from .views import home_view, about_view


urlpatterns = [
    path("", home_view, name="home"),
    path("about/", about_view, name="about"),
]


def register_routes(extra):
    home_view()
    about_view()
    return extra
