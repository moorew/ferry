ARG BASE=ubuntu:24.04
FROM ${BASE}
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
    gnome-shell dbus dbus-user-session python3-gi gir1.2-gtk-4.0 libglib2.0-bin xwayland \
    at-spi2-core fonts-dejavu-core libwebkit2gtk-4.1-0 libgtk-3-0 libayatana-appindicator3-1 \
    && rm -rf /var/lib/apt/lists/*
ARG UID=1000
RUN userdel -r ubuntu 2>/dev/null; useradd -m -u $UID dev
WORKDIR /home/dev
