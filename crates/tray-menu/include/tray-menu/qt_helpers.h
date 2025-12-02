#pragma once

#include "cxx-qt-lib/qpoint.h"
#include "cxx-qt-lib/qstring.h"
#include <QtCore/qcoreapplication.h>
#include <QtCore/qvariant.h>
#include <QtWidgets/qaction.h>
#include <QtWidgets/qapplication.h>
#include <QtWidgets/qmenu.h>

namespace tray_menu_qt {

static int s_argc = 0;
static char *s_argv[] = {nullptr};
static QApplication *s_app = nullptr;

inline bool has_qapplication() {
  return QCoreApplication::instance() != nullptr;
}

inline bool ensure_qapplication() {
  if (QCoreApplication::instance() != nullptr) {
    return true;
  }
  s_app = new QApplication(s_argc, s_argv);
  return s_app != nullptr;
}

inline QMenu *create_menu() { return new QMenu(); }

inline void delete_menu(QMenu *menu) { delete menu; }

inline QAction *add_action(QMenu *menu, const QString &text) {
  return menu->addAction(text);
}

inline void add_separator(QMenu *menu) { menu->addSeparator(); }

inline QMenu *add_submenu(QMenu *menu, const QString &title) {
  return menu->addMenu(title);
}

inline void set_action_enabled(QAction *action, bool enabled) {
  action->setEnabled(enabled);
}

inline void set_action_checkable(QAction *action, bool checkable) {
  action->setCheckable(checkable);
}

inline void set_action_checked(QAction *action, bool checked) {
  action->setChecked(checked);
}

inline void set_action_data(QAction *action, int32_t index) {
  action->setData(QVariant(index));
}

inline int32_t get_action_data(const QAction *action) {
  return action->data().toInt();
}

inline QAction *exec_menu(QMenu *menu, const QPoint &pos) {
  return menu->exec(pos);
}

} // namespace tray_menu_qt
