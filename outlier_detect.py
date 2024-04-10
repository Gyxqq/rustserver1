from PIL import Image
import numpy as np
from scipy.linalg import solve
from scipy.spatial.distance import cdist
import pandas as pd
from sklearn.preprocessing import MinMaxScaler
import sys

# 核心算法
def RLOF(data, k, reg=1e-3):
    n_samples, n_dim = data.shape

    dist = cdist(data, data)
    indices = np.argsort(dist, axis=1)[:, 1:k+1]

    W = np.zeros((n_samples, n_samples))
    v = np.ones(k)

    for i, ind in enumerate(indices):
        A = data[ind]
        C = A - data[i]
        G = np.dot(C, C.T)
        trace = np.trace(G)
        if trace > 0:
            R = reg * trace
        else:
            R = reg
        G.flat[:: k + 1] += R
        w = solve(G, v, assume_a='pos')
        W[i, ind] = w / np.sum(w)
    # print(W)
    # 可达点下标
    reach = np.where(W>1e-10)
  
    n_dist = np.zeros(n_samples)
    for i in range(n_samples):
        # 可达点下标中距离最大的点
        n_dist[i] = np.max(dist[i, reach[1][reach[0]==i]])
        
    # 可达局部可达密度
    lrd = np.zeros(n_samples)
    for i in range(n_samples):
        neighbors = reach[1][reach[0]==i]
        rd = np.maximum(dist[i, neighbors], n_dist[neighbors])
        lrd[i] = len(neighbors) / (np.sum(rd)+1e-10)

    weight = np.sum(W < 1e-10, axis=0)/n_samples

    # 计算局部离群因子
    RLOF = np.zeros(n_samples)
    for i in range(n_samples):
        RLOF[i] = np.mean(np.outer(lrd[reach[1][reach[0]==i]],weight[reach[1][reach[0]==i]]) / lrd[i])

    return RLOF

# 初始化图片为一维数组
def convert_to_num(img_path):
    try:
        if img_path[-3:] == 'png':
            img = Image.open(img_path).convert('RGB')
        elif img_path[-3:] == 'jpg':
            img = Image.open(img_path)
        elif img_path[-4:] == 'jpeg':
            img = Image.open(img_path)
        else:
            print('the image format is not supported', file=sys.stderr)
            return
    except:
        print('can not open the image', file=sys.stderr)
        return
    img = img.resize((64, 64))
    img = np.array(img)
    img = img.reshape(1, -1)
    return img

# 读取数据集
def read_dataset():
    try:
        data = pd.read_csv('data.csv', header=None)
    except:
        # 输出到stderr
        print('can not open the dataset', file=sys.stderr)
        return
    return data

# 异常检测
def outlier_detect(img_path):
    img = convert_to_num(img_path)
    data = read_dataset()
    data = data.iloc[:, :-1]
    data = pd.concat([data, pd.DataFrame(img)], axis=0)
    scaler = MinMaxScaler()
    data = scaler.fit_transform(data)
    rlof = RLOF(data,5)
    # 归一化
    rlof = (rlof - np.min(rlof)) / (np.max(rlof) - np.min(rlof))

    return rlof[-1]

if __name__ == '__main__':
    for img_path in sys.argv[1:]:
        print(outlier_detect(img_path))