// Scan a desktop's or relay's pairing code (SPEC 21.3.3). Also opened by the
// tailcopy://pair link itself, when the system camera scanned the code.

import { CameraView, useCameraPermissions } from 'expo-camera';
import { router, useLocalSearchParams } from 'expo-router';
import { useEffect, useRef } from 'react';
import { Alert, StyleSheet, Text, View } from 'react-native';

import { Core } from '../../modules/tailcopy-core';
import { linkFrom } from '../pairing';
import { t } from '../strings';
import { radius, text, useTheme } from '../theme';
import { Button } from '../ui/Settings';
import { useToast } from '../ui/Toast';

export default function Pair() {
  const { c } = useTheme();
  const toast = useToast();
  const params = useLocalSearchParams<Record<string, string | string[]>>();
  const [permission, ask] = useCameraPermissions();
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
            const paired = await Core.pair(link);
            toast(t('pair.done', { device: paired.name }));
            if (router.canGoBack()) router.back();
            else router.replace('/');
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
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [fromUrl]);

  if (!permission?.granted) {
    return (
      <View style={[styles.center, { backgroundColor: c.bg }]}>
        <Text style={[text('body'), styles.text, { color: c.ink }]}>{t('pair.camera')}</Text>
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
        <Text style={[text('bodyStrong'), { color: c.onSelected }]}>{t('pair.hint')}</Text>
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  fill: { flex: 1 },
  center: { flex: 1, alignItems: 'center', justifyContent: 'center', padding: 32, gap: 20 },
  text: { textAlign: 'center' },
  hint: { position: 'absolute', bottom: 48, alignSelf: 'center', paddingHorizontal: 18, paddingVertical: 12, borderRadius: radius.pill },
});
