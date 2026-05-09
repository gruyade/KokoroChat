# Bugfix Requirements Document

## Introduction

チャットビューにおいて、AIがストリーミングでメッセージを生成している間、ユーザーのスクロール位置に関係なく強制的に最下部へスクロールされるバグの修正。ユーザーが過去のメッセージを読むためにスクロールアップしている場合、自動スクロールを抑制する必要がある。

## Bug Analysis

### Current Behavior (Defect)

1.1 WHEN ユーザーがスクロールアップして過去のメッセージを閲覧中にストリーミングコンテンツが更新される THEN the system が強制的に最下部へスクロールし、ユーザーの閲覧位置が失われる

1.2 WHEN ユーザーがスクロールアップした直後にプログラムによるスムーズスクロールが発生する THEN the system が `isProgrammaticScrollRef` フラグにより `handleScroll` を無視し、`isNearBottomRef` が正しく `false` に更新されない

1.3 WHEN ストリーミング中に `streamingContent` が高頻度で更新される THEN the system が毎回 `smoothScrollToBottom` を呼び出し、`isNearBottomRef` のチェックが競合状態により正しく機能しない

### Expected Behavior (Correct)

2.1 WHEN ユーザーがスクロールアップして過去のメッセージを閲覧中にストリーミングコンテンツが更新される THEN the system SHALL ユーザーのスクロール位置を維持し、自動スクロールを行わない

2.2 WHEN ユーザーが手動でスクロールアップした場合 THEN the system SHALL `isNearBottomRef` を即座に `false` に更新し、以降のプログラムスクロールを抑制する

2.3 WHEN ストリーミング中に `streamingContent` が高頻度で更新される THEN the system SHALL ユーザーが底付近（200px以内）にいる場合のみ自動スクロールを実行する

### Unchanged Behavior (Regression Prevention)

3.1 WHEN ユーザーが最下部付近（200px以内）にいる状態でストリーミングコンテンツが更新される THEN the system SHALL CONTINUE TO スムーズに最下部へ自動スクロールする

3.2 WHEN 新しいメッセージが追加されユーザーが最下部付近にいる THEN the system SHALL CONTINUE TO スムーズに最下部へ自動スクロールする

3.3 WHEN セッションが切り替わる THEN the system SHALL CONTINUE TO 即座に最下部へスクロールする

3.4 WHEN ユーザーが手動で最下部までスクロールダウンする THEN the system SHALL CONTINUE TO 自動スクロールを再度有効にする

---

## Bug Condition (Formal)

### Bug Condition Function

```pascal
FUNCTION isBugCondition(X)
  INPUT: X of type ScrollState { userScrolledUp: boolean, isStreaming: boolean, streamingContentUpdated: boolean }
  OUTPUT: boolean

  // ユーザーがスクロールアップしている状態でストリーミング更新が発生する場合にバグが発生
  RETURN X.userScrolledUp = true AND X.isStreaming = true AND X.streamingContentUpdated = true
END FUNCTION
```

### Property Specification (Fix Checking)

```pascal
// Property: Fix Checking - ユーザーがスクロールアップ中はストリーミング更新で自動スクロールしない
FOR ALL X WHERE isBugCondition(X) DO
  scrollPosition_before ← getScrollPosition()
  onStreamingUpdate(X)
  scrollPosition_after ← getScrollPosition()
  ASSERT scrollPosition_before = scrollPosition_after
END FOR
```

### Preservation Goal

```pascal
// Property: Preservation Checking - ユーザーが底付近にいる場合は従来通り自動スクロール
FOR ALL X WHERE NOT isBugCondition(X) DO
  ASSERT F(X) = F'(X)
END FOR
```
