// First run (SPEC 21.5): welcome, Tailscale, pairing, this phone's name, and an
// honest word on how saving works on a phone. Asked once; never nags.

import { router } from 'expo-router';
import { useState } from 'react';
import { Platform, ScrollView, StyleSheet, Text, TextInput, View } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';

import { Core } from '../../modules/tailcopy-core';
import { useDevices, useSyncStatus } from '../core/hooks';
import { t, type StringKey } from '../strings';
import { radius, text, useTheme } from '../theme';
import { Button } from '../ui/Settings';
import { TailnetBanner } from '../ui/TailnetBanner';

type Step = 'welcome' | 'pair' | 'name' | 'capture';

export default function Onboarding() {
  const { c } = useTheme();
  const insets = useSafeAreaInsets();
  const [chosen, setStep] = useState<Step>('welcome');
  const [devices] = useDevices();
  const status = useSyncStatus();
  const self = devices.find((d) => d.isSelf);
  const paired = devices.some((d) => d.trusted);
  const [edited, setName] = useState<string | null>(null);
  const name = edited ?? self?.name ?? '';
  // Scanning a code on the pairing step moves on by itself.
  const step: Step = chosen === 'pair' && paired ? 'name' : chosen;

  const finish = async (captureOnOpen: boolean) => {
    await Core.setSetting('capture_on_open', captureOnOpen ? 'on' : 'off');
    await Core.setSetting('onboarded', 'yes');
    router.replace('/');
  };

  const title = (key: StringKey) => (
    <Text style={[text('pageTitle'), { color: c.ink }]} accessibilityRole="header">
      {t(key)}
    </Text>
  );
  const body = (s: string) => <Text style={[text('body'), { color: c.ink2 }]}>{s}</Text>;

  return (
    <ScrollView
      style={{ backgroundColor: c.bg }}
      contentContainerStyle={[styles.page, { paddingTop: insets.top + 48, paddingBottom: insets.bottom + 24 }]}
    >
      {step === 'welcome' && (
        <>
          {title('onboarding.welcome.title')}
          {body(t('onboarding.welcome.body'))}
          <View style={styles.spacer} />
          <Button primary title={t('continue')} onPress={() => setStep('pair')} />
        </>
      )}
      {step === 'pair' && (
        <>
          {title('onboarding.pair.title')}
          {body(t('onboarding.pair.body'))}
          {!status.tailnet && <TailnetBanner />}
          <View style={styles.spacer} />
          <Button primary title={t('onboarding.pair.scan')} onPress={() => router.push('/pair')} />
          <Button title={t('skip')} onPress={() => setStep('name')} />
        </>
      )}
      {step === 'name' && (
        <>
          {title('onboarding.name.title')}
          {body(t('onboarding.name.body'))}
          <TextInput
            value={name}
            onChangeText={setName}
            style={[text('search'), styles.input, { color: c.ink, backgroundColor: c.surface }]}
            accessibilityLabel={t('devices.rename')}
            autoFocus
          />
          <View style={styles.spacer} />
          <Button
            primary
            title={t('continue')}
            onPress={async () => {
              if (name.trim()) await Core.renameThisDevice(name);
              setStep('capture');
            }}
          />
        </>
      )}
      {step === 'capture' && (
        <>
          {title('onboarding.capture.title')}
          {body(Platform.OS === 'ios' ? t('settings.capture.ios') : t('settings.capture.android'))}
          {Platform.OS === 'ios' && body(t('settings.capture.ios.allow'))}
          <View style={styles.spacer} />
          <Button primary title={t('settings.capture.on_open')} onPress={() => finish(true)} />
          <Button title={t('onboarding.done')} onPress={() => finish(false)} />
        </>
      )}
    </ScrollView>
  );
}

const styles = StyleSheet.create({
  page: { flexGrow: 1, paddingHorizontal: 24, gap: 16 },
  spacer: { flex: 1, minHeight: 24 },
  input: { paddingHorizontal: 16, paddingVertical: 14, borderRadius: radius.control },
});
