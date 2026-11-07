// History: the picker, and home (SPEC 21.5). On Android it also holds search, the
// filter chips and the way to Devices and Settings, since there is no tab bar.

import { ClipboardPasteButton, isPasteButtonAvailable } from 'expo-clipboard';
import { Stack, router } from 'expo-router';
import { useMemo, useState } from 'react';
import { Platform, Pressable, ScrollView, StyleSheet, Text, View } from 'react-native';

import { Core } from '../../../../modules/tailcopy-core';
import { useClips, useDevices } from '../../../core/hooks';
import { t } from '../../../strings';
import { deviceColour, radius, text, useTheme } from '../../../theme';
import { ClipList } from '../../../ui/ClipList';
import { HeaderButton } from '../../../ui/HeaderButton';
import { TailnetBanner } from '../../../ui/TailnetBanner';
import { useToast } from '../../../ui/Toast';

type Chip = { key: string; label: string; query: string; colour?: string };

export default function History() {
  const { c } = useTheme();
  const toast = useToast();
  const [devices] = useDevices();
  const [chip, setChip] = useState('all');
  const [search, setSearch] = useState('');
  const chips = useMemo<Chip[]>(() => {
    const word = (name: string) => `from:${name.split(/\s+/)[0].toLowerCase()}`;
    const self = devices.find((d) => d.isSelf);
    return [
      { key: 'all', label: t('filter.all'), query: '' },
      ...(Platform.OS === 'android' ? [{ key: 'pinned', label: t('filter.pinned'), query: 'is:pinned' }] : []),
      ...(self ? [{ key: 'here', label: t('filter.here'), query: word(self.name), colour: deviceColour(self.colorIndex) }] : []),
      ...devices
        .filter((d) => !d.isSelf && d.role !== 'relay')
        .map((d) => ({ key: d.id, label: d.name, query: word(d.name), colour: deviceColour(d.colorIndex) })),
      { key: 'images', label: t('filter.images'), query: 'is:image' },
      { key: 'links', label: t('filter.links'), query: 'is:link' },
    ];
  }, [devices]);
  const query = `${chips.find((x) => x.key === chip)?.query ?? ''} ${search}`.trim();
  const [clips] = useClips(query);

  const header = (
    <View>
      <TailnetBanner />
      <ScrollView horizontal showsHorizontalScrollIndicator={false} contentContainerStyle={styles.chips}>
        {chips.map((x) => {
          const on = x.key === chip;
          return (
            <Pressable
              key={x.key}
              onPress={() => setChip(x.key)}
              accessibilityRole="button"
              accessibilityState={{ selected: on }}
              style={[styles.chip, { backgroundColor: on ? c.selected : c.surface, borderColor: c.line }]}
            >
              {x.colour && <View style={[styles.dot, { backgroundColor: x.colour }]} />}
              <Text style={[text('small', 600), { color: on ? c.onSelected : c.ink }]}>{x.label}</Text>
            </Pressable>
          );
        })}
      </ScrollView>
    </View>
  );

  return (
    <>
      <Stack.Screen
        options={{
          title: t('tab.history'),
          headerSearchBarOptions:
            Platform.OS === 'android'
              ? { placeholder: t('search.placeholder'), onChangeText: (e) => setSearch(e.nativeEvent.text) }
              : undefined,
          headerRight: () => (
            <View style={styles.actions}>
              {Platform.OS === 'ios' && isPasteButtonAvailable && (
                <ClipboardPasteButton
                  acceptedContentTypes={['plain-text', 'url']}
                  displayMode="iconOnly"
                  cornerStyle="capsule"
                  style={styles.paste}
                  onPress={async (data) => {
                    if (data.type === 'text' && (await Core.captureText(data.text))) toast(t('toast.saved'));
                  }}
                />
              )}
              {Platform.OS === 'android' && (
                <HeaderButton icon="devices" label={t('tab.devices')} onPress={() => router.push('/devices')} />
              )}
              <HeaderButton icon="settings" label={t('settings.title')} onPress={() => router.push('/settings')} />
            </View>
          ),
        }}
      />
      <ClipList
        clips={clips}
        header={header}
        empty={search ? { title: t('search.empty', { query: search }) } : { title: t('history.empty.title'), body: t('history.empty.body') }}
      />
    </>
  );
}

const styles = StyleSheet.create({
  chips: { gap: 8, paddingHorizontal: 16, paddingVertical: 10 },
  chip: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 6,
    paddingHorizontal: 14,
    minHeight: 36,
    borderRadius: radius.pill,
    borderWidth: StyleSheet.hairlineWidth,
  },
  dot: { width: 8, height: 8, borderRadius: 4 },
  actions: { flexDirection: 'row', alignItems: 'center', gap: 16 },
  paste: { width: 36, height: 36 },
});
