// One clip (SPEC 21.5): type tile, preview, and where it came from (device colour
// and name together). Tap copies. Swipe right pins, swipe left deletes with undo.
// Long-press opens the platform's menu: a native context menu with a preview on
// iOS, a sheet on Android.

import * as Haptics from 'expo-haptics';
import { Link, router } from 'expo-router';
import { memo, useRef } from 'react';
import { Platform, Pressable, Share, StyleSheet, Text, View } from 'react-native';
import ReanimatedSwipeable, { type SwipeableMethods } from 'react-native-gesture-handler/ReanimatedSwipeable';

import { Core, type ClipItem } from '../../modules/tailcopy-core';
import { age } from '../format';
import { t } from '../strings';
import { deviceColour, radius, text, useTheme } from '../theme';
import { Icon, type IconName } from './Icon';
import { useToast } from './Toast';

const lookIcon: Record<ClipItem['look'], IconName> = {
  text: 'text',
  rich: 'rich-text',
  image: 'image',
  code: 'code',
  link: 'link',
  colour: 'colour',
};

export function useClipActions() {
  const toast = useToast();
  return {
    copy: async (clip: ClipItem, plain = false) => {
      const ok = await Core.copy(clip.id, plain).catch(() => false);
      if (ok) {
        Haptics.selectionAsync();
        toast(t('toast.copied'));
      } else {
        toast(t('toast.unavailable'));
      }
    },
    pin: (clip: ClipItem) => Core.setPinned(clip.id, !clip.pinned),
    remove: async (clip: ClipItem) => {
      await Core.remove(clip.id);
      toast(t('toast.deleted'), () => {
        // Deleting syncs a tombstone; undo saves the text again as a new clip.
        if (clip.kind !== 'image') Core.captureText(clip.preview);
      });
    },
    share: async (clip: ClipItem) => {
      const detail = await Core.detail(clip.id);
      if (detail.text) await Share.share({ message: detail.text });
    },
  };
}

function Row({ clip }: { clip: ClipItem }) {
  const { c } = useTheme();
  const actions = useClipActions();
  const swipe = useRef<SwipeableMethods>(null);
  const device = clip.fromThisDevice ? t('row.here') : clip.deviceName;
  const when = age(clip.createdMs);
  const label = t(clip.pinned ? 'row.a11y.pinned' : 'row.a11y', {
    preview: clip.label ?? clip.preview,
    device,
    age: when,
  });

  const body = (
    <View style={[styles.row, { backgroundColor: c.surface }]}>
      <View style={[styles.tile, { backgroundColor: c.tile }]}>
        <Icon name={lookIcon[clip.look]} size={18} color={c.ink2} />
      </View>
      <View style={styles.main}>
        {clip.label && (
          <Text style={[text('meta', 600), { color: c.accentInk }]} numberOfLines={1}>
            {clip.label}
          </Text>
        )}
        <Text style={[text(clip.mono ? 'rowCode' : 'body'), { color: c.ink }]} numberOfLines={2}>
          {clip.preview || ' '}
        </Text>
        <View style={styles.meta}>
          <View style={[styles.dot, { backgroundColor: deviceColour(clip.colorIndex) }]} />
          <Text style={[text('meta'), { color: c.ink3 }]} numberOfLines={1}>
            {device} · {when}
            {clip.localOnly ? ` · ${t('row.not_synced')}` : ''}
          </Text>
        </View>
      </View>
      {clip.pinned && <Icon name="pin" size={16} color={c.accentInk} />}
    </View>
  );

  const pressable =
    Platform.OS === 'ios' ? (
      <Link href={`/clip/${clip.id}`} asChild>
        <Link.Trigger>
          <Pressable
            onPress={(e) => {
              e.preventDefault();
              actions.copy(clip);
            }}
            accessibilityRole="button"
            accessibilityLabel={label}
            accessibilityHint={t('action.copy')}
          >
            {body}
          </Pressable>
        </Link.Trigger>
        <Link.Preview />
        <Link.Menu>
          <Link.MenuAction title={t('action.copy')} onPress={() => actions.copy(clip)} />
          <Link.MenuAction title={t('action.copy_plain')} onPress={() => actions.copy(clip, true)} />
          <Link.MenuAction title={t(clip.pinned ? 'action.unpin' : 'action.pin')} onPress={() => actions.pin(clip)} />
          <Link.MenuAction title={t('action.details')} onPress={() => router.push(`/clip/${clip.id}`)} />
          <Link.MenuAction title={t('action.share')} onPress={() => actions.share(clip)} />
          <Link.MenuAction title={t('action.delete')} destructive onPress={() => actions.remove(clip)} />
        </Link.Menu>
      </Link>
    ) : (
      <Pressable
        onPress={() => actions.copy(clip)}
        onLongPress={() => router.push(`/clip/${clip.id}`)}
        android_ripple={{ color: c.hover }}
        accessibilityRole="button"
        accessibilityLabel={label}
        accessibilityActions={[
          { name: 'pin', label: t(clip.pinned ? 'action.unpin' : 'action.pin') },
          { name: 'delete', label: t('action.delete') },
        ]}
        onAccessibilityAction={(e) =>
          e.nativeEvent.actionName === 'pin' ? actions.pin(clip) : actions.remove(clip)
        }
      >
        {body}
      </Pressable>
    );

  return (
    <ReanimatedSwipeable
      ref={swipe}
      friction={1.5}
      leftThreshold={72}
      rightThreshold={72}
      renderLeftActions={() => (
        <View style={[styles.action, { backgroundColor: c.accent }]}>
          <Icon name="pin" color={c.onAccent} />
        </View>
      )}
      renderRightActions={() => (
        <View style={[styles.action, styles.right, { backgroundColor: c.danger }]}>
          <Icon name="delete" color={c.onAccent} />
        </View>
      )}
      onSwipeableOpen={(direction) => {
        swipe.current?.close();
        Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Light);
        // The direction the row moved: right shows the pin action on the left.
        if (direction === 'right') actions.pin(clip);
        else actions.remove(clip);
      }}
    >
      {pressable}
    </ReanimatedSwipeable>
  );
}

export const ClipRow = memo(Row);

const styles = StyleSheet.create({
  row: { flexDirection: 'row', alignItems: 'center', gap: 12, paddingHorizontal: 16, paddingVertical: 12 },
  tile: { width: 36, height: 36, borderRadius: radius.tile, alignItems: 'center', justifyContent: 'center' },
  main: { flex: 1, gap: 3 },
  meta: { flexDirection: 'row', alignItems: 'center', gap: 6 },
  dot: { width: 8, height: 8, borderRadius: radius.dot * 2 },
  action: { flex: 1, justifyContent: 'center', paddingHorizontal: 24 },
  right: { alignItems: 'flex-end' },
});
