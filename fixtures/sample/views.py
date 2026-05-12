"""Django-style views module for the about page."""
from django.http import HttpResponse
from django.shortcuts import render
import json


# Renders the public landing page.
def home_view(request):
    return render(request, "home.html")


def about_view(request):
    """Return the static about-us page."""
    return render(request, "about.html")


class ContactHandler:
    def handle_post(self, request):
        return HttpResponse("ok")
