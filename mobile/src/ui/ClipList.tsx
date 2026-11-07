// A list of clips, newest first, with an empty state.

import { LegendList } from '@legendapp/list/react-native';
import type { ReactElement } from 'react';
import { StyleSheet, Text, View } from 'react-native';

import type { ClipItem } from '../../modules/tailcopy-core';
import { text, useTheme } from '../theme';
import { ClipRow } from './ClipRow';

export function ClipList({
  clips,
  empty,
  header,
}: {
  clips: ClipItem[];
  empty: { title: string; body?: string };
  header?: ReactElement;
}) {
  const { c } = useTheme();
  return (
    <LegendList
      data={clips}
      keyExtractor={(clip) => clip.id}
      renderItem={({ item }) => <ClipRow clip={item} />}
      recycleItems
      estimatedItemSize={72}
      contentInsetAdjustmentBehavior="automatic"
      style={{ backgroundColor: c.bg }}
      ListHeaderComponent={header}
      ItemSeparatorComponent={() => <View style={[styles.separator, { backgroundColor: c.line }]} />}
      ListEmptyComponent={
        <View style={styles.empty}>
          <Text style={[text('stateTitle'), { color: c.ink }]}>{empty.title}</Text>
          {empty.body && <Text style={[text('body'), styles.center, { color: c.ink2 }]}>{empty.body}</Text>}
        </View>
      }
    />
  );
}

const styles = StyleSheet.create({
  separator: { height: StyleSheet.hairlineWidth, marginLeft: 64 },
  empty: { padding: 32, gap: 12, alignItems: 'center' },
  center: { textAlign: 'center' },
});
