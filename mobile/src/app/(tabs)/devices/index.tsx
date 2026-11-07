// Devices (SPEC 21.5): this phone's name, then every desktop and relay it trusts,
// with their sync state and switches, and pairing.

import { Stack, router } from 'expo-router';
import { useState } from 'react';
import { Alert, ScrollView, StyleSheet, TextInput, View } from 'react-native';

import { Core, type DeviceItem } from '../../../../modules/tailcopy-core';
import { useDevices } from '../../../core/hooks';
import { age } from '../../../format';
import { t } from '../../../strings';
import { deviceColour, radius, text, useTheme } from '../../../theme';
import { Icon } from '../../../ui/Icon';
import { Button, Item, Section, Toggle } from '../../../ui/Settings';
import { TailnetBanner } from '../../../ui/TailnetBanner';

function state(d: DeviceItem): string {
  if (d.error) return d.error;
  if (d.connected) return d.caughtUp ? t('devices.synced') : t('devices.syncing');
  return d.lastSyncMs ? t('devices.last', { ago: age(d.lastSyncMs) }) : t('devices.never');
}

function Avatar({ d }: { d: DeviceItem }) {
  const { c } = useTheme();
  const icon = d.role === 'relay' ? 'server' : d.role === 'mobile' ? 'phone' : 'desktop';
  return (
    <View style={[styles.avatar, { backgroundColor: deviceColour(d.colorIndex) }]}>
      <Icon name={icon} size={17} color={c.glyphOnDevice} />
    </View>
  );
}

export default function Devices() {
  const { c } = useTheme();
  const [devices, reload] = useDevices();
  const self = devices.find((d) => d.isSelf);
  const others = devices.filter((d) => !d.isSelf && d.role !== 'mobile');
  // What the user typed, or the stored name until they type.
  const [edited, setName] = useState<string | null>(null);
  const name = edited ?? self?.name ?? '';

  const forget = (d: DeviceItem) =>
    Alert.alert(t('devices.forget.title', { device: d.name }), t('devices.forget.body'), [
      { text: t('cancel'), style: 'cancel' },
      { text: t('devices.forget'), style: 'destructive', onPress: () => Core.forget(d.id).then(reload) },
    ]);

  return (
    <>
      <Stack.Screen options={{ title: t('tab.devices') }} />
      <ScrollView contentInsetAdjustmentBehavior="automatic" contentContainerStyle={styles.page}>
        <TailnetBanner />
        {self && (
          <Section title={t('devices.this')}>
            <Item
              leading={<Avatar d={self} />}
              title={t('devices.rename')}
              trailing={
                <TextInput
                  value={name}
                  onChangeText={setName}
                  onEndEditing={() => name.trim() && Core.renameThisDevice(name)}
                  style={[text('body'), styles.name, { color: c.ink, backgroundColor: c.sunk }]}
                  accessibilityLabel={t('devices.rename')}
                  returnKeyType="done"
                />
              }
            />
          </Section>
        )}
        <Section title={t('devices.paired')} footer={others.length ? undefined : t('devices.none')}>
          {others.map((d) => (
            <View key={d.id}>
              <Item
                leading={<Avatar d={d} />}
                title={d.role === 'relay' ? `${d.name} · ${t('devices.relay')}` : d.name}
                detail={state(d)}
                onPress={() => forget(d)}
              />
              <Item
                title={t('devices.send')}
                trailing={<Toggle value={d.send} label={`${t('devices.send')} ${d.name}`} onChange={(v) => Core.setDeviceSync(d.id, v, d.receive).then(reload)} />}
              />
              <Item
                title={t('devices.receive')}
                trailing={<Toggle value={d.receive} label={`${t('devices.receive')} ${d.name}`} onChange={(v) => Core.setDeviceSync(d.id, d.send, v).then(reload)} />}
              />
            </View>
          ))}
        </Section>
        <View style={styles.pair}>
          <Button primary title={t('devices.pair')} onPress={() => router.push('/pair')} />
        </View>
      </ScrollView>
    </>
  );
}

const styles = StyleSheet.create({
  page: { paddingBottom: 48 },
  avatar: { width: 28, height: 28, borderRadius: radius.avatar[28], alignItems: 'center', justifyContent: 'center' },
  name: { minWidth: 140, paddingHorizontal: 12, paddingVertical: 8, borderRadius: radius.control },
  pair: { margin: 16, marginTop: 24 },
});
