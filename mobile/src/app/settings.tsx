// Settings (SPEC 21.5, 21.6): capture, privacy, storage and about. Paired devices
// live on the Devices screen.

import Constants from 'expo-constants';
import { router } from 'expo-router';
import { Alert, Platform, ScrollView, StyleSheet, Text } from 'react-native';

import { Core } from '../../modules/tailcopy-core';
import { useSetting } from '../core/hooks';
import { t } from '../strings';
import { text, useTheme } from '../theme';
import { Item, Section, Toggle } from '../ui/Settings';

export default function Settings() {
  const { c } = useTheme();
  const [onOpen, setOnOpen] = useSetting('capture_on_open');
  const [mirror, setMirror] = useSetting('live_mirror');
  const ios = Platform.OS === 'ios';

  const clear = () =>
    Alert.alert(t('settings.clear.title'), t('settings.clear.body'), [
      { text: t('cancel'), style: 'cancel' },
      { text: t('settings.clear'), style: 'destructive', onPress: () => Core.clearHistory(false) },
    ]);

  return (
    <ScrollView contentInsetAdjustmentBehavior="automatic" contentContainerStyle={styles.page}>
      <Section title={t('settings.capture')} footer={ios ? t('settings.capture.ios.allow') : undefined}>
        <Item
          title={t('settings.capture.on_open')}
          trailing={<Toggle value={onOpen === 'on'} label={t('settings.capture.on_open')} onChange={(v) => setOnOpen(v ? 'on' : 'off')} />}
        />
        <Item title={ios ? t('settings.capture.ios') : t('settings.capture.android')} />
      </Section>
      <Section title={t('settings.privacy')} footer={ios ? t('settings.secrets.ios') : t('settings.secrets.android')}>
        <Item
          title={t('settings.mirror')}
          detail={t('settings.mirror.desc')}
          trailing={<Toggle value={mirror === 'on'} label={t('settings.mirror')} onChange={(v) => setMirror(v ? 'on' : 'off')} />}
        />
      </Section>
      <Section title={t('tab.devices')}>
        <Item title={t('devices.paired')} onPress={() => router.push('/devices')} />
      </Section>
      <Section title={t('settings.storage')}>
        <Item title={t('settings.clear')} destructive onPress={clear} />
      </Section>
      <Section title={t('settings.about')}>
        <Item title={t('settings.about.body')} detail={t('settings.version', { version: Constants.expoConfig?.version ?? '' })} />
      </Section>
      <Text style={[text('meta'), styles.licence, { color: c.ink3 }]}>MIT OR Apache-2.0</Text>
    </ScrollView>
  );
}

const styles = StyleSheet.create({
  page: { paddingBottom: 48 },
  licence: { textAlign: 'center', marginTop: 24 },
});
