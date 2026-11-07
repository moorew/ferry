// Root: theme, toasts, the onboarding gate, and saving the clipboard whenever the
// app comes to the front, if the user turned that on (SPEC 21.4).

import { Stack, router, useRootNavigationState } from 'expo-router';
import { useEffect } from 'react';
import { AppState } from 'react-native';
import { GestureHandlerRootView } from 'react-native-gesture-handler';

import { Core } from '../../modules/tailcopy-core';
import { t, type StringKey } from '../strings';
import { ThemeProvider, useTheme } from '../theme';
import { ToastProvider, useToast } from '../ui/Toast';

const captureToasts: Partial<Record<string, StringKey>> = {
  saved: 'toast.saved',
  sensitive: 'toast.sensitive',
  denied: 'toast.denied',
};

function CaptureOnOpen() {
  const toast = useToast();
  useEffect(() => {
    const capture = async () => {
      if ((await Core.setting('capture_on_open')) !== 'on') return;
      const result = await Core.captureClipboard().catch(() => 'denied' as const);
      const key = captureToasts[result];
      if (key) toast(t(key));
    };
    capture();
    const sub = AppState.addEventListener('change', (s) => s === 'active' && capture());
    return () => sub.remove();
  }, [toast]);
  return null;
}

function Onboarding() {
  const nav = useRootNavigationState();
  useEffect(() => {
    if (!nav?.key) return;
    Core.setting('onboarded').then((v) => {
      if (v !== 'yes') router.replace('/onboarding');
    });
  }, [nav?.key]);
  return null;
}

function Screens() {
  const { c } = useTheme();
  return (
    <Stack
      screenOptions={{
        headerTintColor: c.accentInk,
        contentStyle: { backgroundColor: c.bg },
        headerShadowVisible: false,
      }}
    >
      <Stack.Screen name="(tabs)" options={{ headerShown: false }} />
      <Stack.Screen name="clip/[id]" options={{ title: t('detail.title'), presentation: 'formSheet', sheetAllowedDetents: [0.6, 1], sheetGrabberVisible: true }} />
      <Stack.Screen name="settings" options={{ title: t('settings.title'), presentation: 'modal' }} />
      <Stack.Screen name="pair" options={{ title: t('pair.title'), presentation: 'fullScreenModal' }} />
      <Stack.Screen name="onboarding" options={{ headerShown: false, gestureEnabled: false }} />
      <Stack.Screen name="capture" options={{ headerShown: false, presentation: 'transparentModal', animation: 'fade' }} />
      <Stack.Screen name="copy-latest" options={{ headerShown: false, presentation: 'transparentModal', animation: 'fade' }} />
    </Stack>
  );
}

export default function Root() {
  return (
    <GestureHandlerRootView style={{ flex: 1 }}>
      <ThemeProvider>
        <ToastProvider>
          <Screens />
          <Onboarding />
          <CaptureOnOpen />
        </ToastProvider>
      </ThemeProvider>
    </GestureHandlerRootView>
  );
}
