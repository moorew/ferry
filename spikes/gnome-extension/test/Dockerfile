ARG BASE=ubuntu:24.04
FROM ${BASE}
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
    gnome-shell gnome-shell-extension-prefs dbus dbus-user-session python3-gi gir1.2-gtk-3.0 \
    libglib2.0-bin xwayland at-spi2-core fonts-dejavu-core \
    && rm -rf /var/lib/apt/lists/*
ARG UID=1000
RUN userdel -r ubuntu 2>/dev/null; useradd -m -u $UID dev
USER dev
WORKDIR /home/dev
