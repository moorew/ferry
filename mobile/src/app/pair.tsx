// Scan a desktop's or relay's pairing code (SPEC 21.3.3). Also opened by the
// tailcopy://pair link itself, when the system camera scanned the code. After the
// code is accepted, the screen follows the first connection until history is in.

import { CameraView, useCameraPermissions } from 'expo-camera';
import { router, useLocalSearchParams } from 'expo-router';
import { useEffect, useRef, useState } from 'react';
import { Alert, ScrollView, StyleSheet, Text, View } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';

import { Core } from '../../modules/tailcopy-core';
import { useDevices, useSyncStatus } from '../core/hooks';
import { linkFrom } from '../pairing';
import { t } from '../strings';
import { radius, text, useTheme } from '../theme';
import { Icon, type IconName } from '../ui/Icon';
import { Button } from '../ui/Settings';
import { TailnetBanner } from '../ui/TailnetBanner';

/** How long to wait for the first connection before suggesting what to check. */
const SLOW_MS = 15_000;

export default function Pair() {
  const { c } = useTheme();
  const params = useLocalSearchParams<Record<string, string | string[]>>();
  const [permission, ask] = useCameraPermissions();
  const [paired, setPaired] = useState<{ deviceId: string; name: string } | null>(null);
  const busy = useRef(false);

  const offer = (link: string) => {
    if (busy.current) return;
    busy.current = true;
    const name = new URL(link.replace('tailcopy://', 'https://')).searchParams.get('name') || '?';
    Alert.alert(t('pair.confirm.title', { device: name }), t('pair.confirm.body', { device: name }), [
      { text: t('cancel'), style: 'cancel', onPress: () => (busy.current = false) },
      {
        text: t('pair.confirm.ok'),
        onPress: async () => {
          try {
            setPaired(await Core.pair(link));
          } catch (e) {
            busy.current = false;
            Alert.alert(String(e instanceof Error ? e.message : e));
          }
        },
      },
    ]);
  };

  const fromUrl = linkFrom(params);
  useEffect(() => {
    if (fromUrl) offer(fromUrl);
    // Offer once per link.
  }, [fromUrl]);

  if (paired) return <Progress deviceId={paired.deviceId} name={paired.name} />;

  if (!permission?.granted) {
    return (
      <View style={[styles.center, { backgroundColor: c.bg }]}>
        <Text style={[text('body'), styles.centred, { color: c.ink }]}>{t('pair.camera')}</Text>
        <Button primary title={t('pair.camera.allow')} onPress={ask} />
      </View>
    );
  }
  return (
    <View style={styles.fill}>
      <CameraView
        style={styles.fill}
        facing="back"
        barcodeScannerSettings={{ barcodeTypes: ['qr'] }}
        onBarcodeScanned={({ data }) => data.startsWith('tailcopy://pair') && offer(data)}
      />
      <View style={[styles.hint, { backgroundColor: c.selected }]}>
        <Text style={[text('bodyStrong'), styles.centred, { color: c.onSelected }]}>{t('pair.hint')}</Text>
      </View>
    </View>
  );
}

type StepState = 'done' | 'active' | 'waiting' | 'failed';

function Progress({ deviceId, name }: { deviceId: string; name: string }) {
  const { c } = useTheme();
  const insets = useSafeAreaInsets();
  const [devices] = useDevices();
  const status = useSyncStatus();
  const [slow, setSlow] = useState(false);

  useEffect(() => {
    const timer = setTimeout(() => setSlow(true), SLOW_MS);
    return () => clearTimeout(timer);
  }, []);

  const peer = devices.find((d) => d.id === deviceId);
  const connected = !!peer?.connected;
  const caughtUp = connected && !!peer?.caughtUp;
  const others = devices.filter((d) => !d.isSelf && d.trusted && d.id !== deviceId).length;
  const failed = !connected && peer?.error ? peer.error : null;

  const connectState: StepState = connected ? 'done' : failed ? 'failed' : 'active';
  const syncState: StepState = caughtUp ? 'done' : connected ? 'active' : 'waiting';

  const close = () => (router.canGoBack() ? router.back() : router.replace('/'));

  return (
    <ScrollView
      style={{ backgroundColor: c.bg }}
      contentContainerStyle={[styles.page, { paddingTop: insets.top + 32, paddingBottom: insets.bottom + 24 }]}
    >
      <Text style={[text('pageTitle'), { color: c.ink }]} accessibilityRole="header">
        {caughtUp ? t('pair.progress.done_title') : t('pair.progress.title', { device: name })}
      </Text>
      <View style={styles.steps} accessibilityLiveRegion="polite">
        <Step state="done" label={t('pair.progress.trusted', { device: name })} />
        <Step
          state={connectState}
          label={connected ? t('pair.progress.connected', { device: name }) : t('pair.progress.connecting', { device: name })}
          detail={failed ?? undefined}
        />
        <Step
          state={syncState}
          label={caughtUp ? t('pair.progress.synced') : t('pair.progress.syncing')}
          detail={caughtUp && others > 0 ? t('pair.progress.others', { n: others }) : undefined}
        />
      </View>
      {!status.tailnet && <TailnetBanner />}
      {slow && !connected && status.tailnet && (
        <View style={[styles.note, { backgroundColor: c.sunk }]}>
          <Text style={[text('bodyStrong'), { color: c.ink }]}>{t('pair.progress.slow.title')}</Text>
          <Text style={[text('small'), { color: c.ink2 }]}>{t('pair.progress.slow.body', { device: name })}</Text>
          <Button title={t('tailscale.retry')} onPress={() => Core.retry()} />
        </View>
      )}
      <View style={styles.spacer} />
      <Button primary={caughtUp} title={caughtUp ? t('pair.progress.finish') : t('pair.progress.background')} onPress={close} />
    </ScrollView>
  );
}

function Step({ state, label, detail }: { state: StepState; label: string; detail?: string }) {
  const { c } = useTheme();
  const icons: Record<StepState, IconName> = { done: 'check', active: 'syncing', waiting: 'syncing', failed: 'offline' };
  const tint = { done: c.accentInk, active: c.ink, waiting: c.ink3, failed: c.danger }[state];
  return (
    <View style={styles.step}>
      <View style={[styles.badge, { backgroundColor: state === 'done' ? c.accentSoft : c.sunk }]}>
        <Icon name={icons[state]} size={18} color={tint} />
      </View>
      <View style={styles.stepText}>
        <Text style={[text(state === 'waiting' ? 'body' : 'bodyStrong'), { color: state === 'waiting' ? c.ink3 : c.ink }]}>{label}</Text>
        {detail && <Text style={[text('small'), { color: state === 'failed' ? c.danger : c.ink2 }]}>{detail}</Text>}
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  fill: { flex: 1 },
  center: { flex: 1, alignItems: 'center', justifyContent: 'center', padding: 32, gap: 20 },
  centred: { textAlign: 'center' },
  hint: { position: 'absolute', bottom: 48, left: 24, right: 24, alignItems: 'center', paddingHorizontal: 18, paddingVertical: 12, borderRadius: radius.card },
  page: { flexGrow: 1, paddingHorizontal: 24, gap: 24 },
  steps: { gap: 18 },
  step: { flexDirection: 'row', alignItems: 'flex-start', gap: 14 },
  badge: { width: 34, height: 34, alignItems: 'center', justifyContent: 'center', borderRadius: radius.pill },
  stepText: { flex: 1, gap: 2, paddingTop: 6 },
  note: { padding: 16, gap: 10, borderRadius: radius.card },
  spacer: { flex: 1, minHeight: 24 },
});
