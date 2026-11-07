# Builds the Android app without a local SDK (SPEC 21.7):
#   docker build -t tailcopy-android -f mobile/docker/android.Dockerfile mobile/docker
#   docker run --rm -v "$PWD":/src -w /src/mobile tailcopy-android \
#     bash -c 'npm ci && npm run rust:android && npx expo prebuild -p android --clean && cd android && ./gradlew assembleRelease'
FROM rust:1.99
SHELL ["/bin/bash", "-o", "pipefail", "-c"]

ARG CMDLINE_TOOLS=commandlinetools-linux-9862592_latest.zip
ARG NODE=v22.23.4
ENV ANDROID_HOME=/opt/android-sdk \
    ANDROID_NDK_VERSION=27.1.12297006 \
    EXPO_NO_TELEMETRY=1 \
    GRADLE_OPTS="-Dorg.gradle.daemon=false -Dorg.gradle.jvmargs=-Xmx4g"
ENV ANDROID_NDK_HOME=$ANDROID_HOME/ndk/$ANDROID_NDK_VERSION \
    PATH=/opt/node/bin:$ANDROID_HOME/cmdline-tools/latest/bin:$ANDROID_HOME/platform-tools:$PATH

RUN apt-get update && apt-get install -y --no-install-recommends openjdk-21-jdk-headless unzip \
    && rm -rf /var/lib/apt/lists/*

RUN curl -fsSL https://nodejs.org/dist/$NODE/node-$NODE-linux-x64.tar.xz | tar -xJ -C /opt \
    && mv /opt/node-$NODE-linux-x64 /opt/node

RUN mkdir -p $ANDROID_HOME/cmdline-tools \
    && curl -fsSLo /tmp/tools.zip https://dl.google.com/android/repository/$CMDLINE_TOOLS \
    && unzip -q /tmp/tools.zip -d $ANDROID_HOME/cmdline-tools \
    && mv $ANDROID_HOME/cmdline-tools/cmdline-tools $ANDROID_HOME/cmdline-tools/latest \
    && rm /tmp/tools.zip \
    && sdkmanager --licenses < <(yes) > /dev/null \
    && sdkmanager "platform-tools" "platforms;android-37.0" "platforms;android-36" "build-tools;37.0.0" "build-tools;36.0.0" "ndk;$ANDROID_NDK_VERSION" "cmake;3.22.1" | tail -1

RUN rustup target add aarch64-linux-android x86_64-linux-android \
    && cargo install cargo-ndk --version 4.1.2 --locked
