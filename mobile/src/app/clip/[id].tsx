// One clip in full (SPEC 21.5): the text, where it came from, its label, actions.

import { Stack, router, useLocalSearchParams } from 'expo-router';
import { useEffect, useState } from 'react';
import { ScrollView, StyleSheet, Text, TextInput, View } from 'react-native';
import { Image } from 'expo-image';

import { Core, type ClipItem, type ClipText } from '../../../modules/tailcopy-core';
import { useClips } from '../../core/hooks';
import { age } from '../../format';
import { t } from '../../strings';
import { deviceColour, radius, text, useTheme } from '../../theme';
import { useClipActions } from '../../ui/ClipRow';
import { Button } from '../../ui/Settings';

export default function Clip() {
  const { c } = useTheme();
  const { id } = useLocalSearchParams<{ id: string }>();
  const [clips] = useClips('', 500);
  const clip: ClipItem | undefined = clips.find((x) => x.id === id);
  const [detail, setDetail] = useState<ClipText | null>(null);
  const [edited, setLabel] = useState<string | null>(null);
  const actions = useClipActions();
  useEffect(() => {
    Core.detail(id).then(setDetail, () => setDetail(null));
  }, [id]);
  if (!clip) return null;
  const label = edited ?? clip.label ?? '';
  const device = clip.fromThisDevice ? t('row.here') : clip.deviceName;

  return (
    <>
      <Stack.Screen options={{ title: clip.label ?? t('detail.title') }} />
      <ScrollView contentContainerStyle={styles.page}>
        <View style={styles.from}>
          <View style={[styles.dot, { backgroundColor: deviceColour(clip.colorIndex) }]} />
          <Text style={[text('meta'), { color: c.ink2 }]}>{t('detail.from', { device, age: age(clip.createdMs) })}</Text>
        </View>
        <TextInput
          value={label}
          onChangeText={setLabel}
          onEndEditing={() => Core.setLabel(clip.id, label.trim() || null)}
          placeholder={t('detail.label.placeholder')}
          placeholderTextColor={c.ink3}
          style={[text('setting'), styles.label, { color: c.accentInk, backgroundColor: c.sunk }]}
          accessibilityLabel={t('action.label')}
        />
        {clip.thumbnail && <Image source={{ uri: clip.thumbnail }} style={styles.image} contentFit="contain" accessibilityLabel={t('detail.image')} />}
        {detail?.text != null && (
          <View style={[styles.card, { backgroundColor: c.surface }]}>
            <Text selectable style={[text(clip.mono ? 'code' : 'body'), { color: c.ink }]}>
              {detail.text}
            </Text>
          </View>
        )}
        {detail?.incomplete && <Text style={[text('small'), { color: c.ink2 }]}>{t('detail.incomplete', { device })}</Text>}
        <View style={styles.buttons}>
          <Button primary title={t('action.copy')} onPress={() => actions.copy(clip)} />
          {clip.kind !== 'image' && clip.kind !== 'text' && <Button title={t('action.copy_plain')} onPress={() => actions.copy(clip, true)} />}
          <Button title={t(clip.pinned ? 'action.unpin' : 'action.pin')} onPress={() => actions.pin(clip)} />
          <Button title={t('action.share')} onPress={() => actions.share(clip)} />
          <Button
            title={t('action.delete')}
            onPress={() => {
              actions.remove(clip);
              router.back();
            }}
          />
        </View>
      </ScrollView>
    </>
  );
}

const styles = StyleSheet.create({
  page: { padding: 16, gap: 14 },
  from: { flexDirection: 'row', alignItems: 'center', gap: 8 },
  dot: { width: 8, height: 8, borderRadius: 4 },
  label: { paddingHorizontal: 14, paddingVertical: 10, borderRadius: radius.control },
  card: { padding: 16, borderRadius: radius.card },
  image: { width: '100%', aspectRatio: 1.4, borderRadius: radius.card },
  buttons: { gap: 10 },
});
